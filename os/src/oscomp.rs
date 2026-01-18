//! OSCOMP mode entrypoint.
//!
//! When the `oscomp` feature is enabled, the kernel skips BusyBox init and
//! enters this path instead.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::arch::lib::sbi::shutdown;
use crate::device::CMDLINE;
use crate::ipc::{SignalHandlerTable, SignalPending};
use crate::kernel::{
    Scheduler, TASK_MANAGER, TaskManagerTrait, TaskState, TaskStruct, current_cpu, current_task,
    pick_cpu, prepare_exec_image_from_path, scheduler_of, yield_task,
};
use crate::mm::frame_allocator::{alloc_contig_frames, alloc_frame};
use crate::sync::SpinLock;
use crate::vfs::{
    File, FileMode, FsError, InodeMetadata, InodeType, create_stdio_files, get_root_dentry,
    vfs_lookup, vfs_ops,
};
use uapi::signal::{SignalFlags, SignalStack};
use uapi::uts_namespace::{UTS_NAME_LEN, UtsNamespace};

/// EOF-only stdin to avoid interactive shells blocking on terminal input after scripts.
struct NullStdin;

impl File for NullStdin {
    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Ok(0)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let now = vfs_ops().timespec_now();
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IRUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: now,
            mtime: now,
            ctime: now,
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

fn ensure_top_level_dir(path: &str) -> Result<(), FsError> {
    let mode = FileMode::S_IFDIR | FileMode::from_bits_truncate(0o755);
    if vfs_lookup(path).is_ok() {
        return Ok(());
    }
    let name = path.strip_prefix('/').ok_or(FsError::InvalidArgument)?;
    if name.is_empty() || name.contains('/') {
        return Err(FsError::InvalidArgument);
    }
    let root = get_root_dentry()?;
    root.inode.mkdir(name, mode)?;
    Ok(())
}

fn setup_minimal_mounts() {
    // Ensure mount points exist (rootfs should provide them, but don't assume).
    let _ = ensure_top_level_dir("/dev");
    let _ = ensure_top_level_dir("/proc");
    let _ = ensure_top_level_dir("/sys");
    let _ = ensure_top_level_dir("/tmp");
    let _ = ensure_top_level_dir("/tests");

    // /dev: tmpfs + device nodes (console/null/zero/tty/vda...)
    if let Err(e) = crate::fs::mount_tmpfs("/dev", 0) {
        crate::pr_warn!("[OSCOMP] mount tmpfs /dev failed: {:?}", e);
    } else if let Err(e) = crate::fs::init_dev() {
        crate::pr_warn!("[OSCOMP] init /dev nodes failed: {:?}", e);
    }

    // /proc and /sys are optional for scoring, but many userland tools expect them.
    if let Err(e) = crate::fs::init_procfs() {
        crate::pr_warn!("[OSCOMP] mount procfs failed: {:?}", e);
    }
    if let Err(e) = crate::fs::init_sysfs() {
        crate::pr_warn!("[OSCOMP] mount sysfs failed: {:?}", e);
    }

    // /tmp as tmpfs (best-effort).
    if let Err(e) = crate::fs::mount_tmpfs("/tmp", 0) {
        crate::pr_warn!("[OSCOMP] mount tmpfs /tmp failed: {:?}", e);
    }
}

fn pick_tests_root() -> &'static str {
    // In the intended judge setup, test scripts live in the root of the x0 disk mounted at /tests.
    // If /tests isn't mounted for any reason, fall back to scanning "/".
    if let Ok(d) = vfs_lookup("/tests") {
        if d.inode.metadata().map(|m| m.inode_type).ok() == Some(crate::vfs::InodeType::Directory) {
            return "/tests";
        }
    }
    "/"
}

fn list_test_scripts(tests_root: &str) -> Vec<(String, String)> {
    let Ok(dentry) = vfs_lookup(tests_root) else {
        crate::pr_warn!("[OSCOMP] {} not found; no tests to run", tests_root);
        return Vec::new();
    };
    let Ok(entries) = dentry.inode.readdir() else {
        crate::pr_warn!("[OSCOMP] readdir({}) failed; no tests to run", tests_root);
        return Vec::new();
    };

    let mut scripts: Vec<(String, String)> = Vec::new();

    // 1) Scripts directly under tests_root.
    for e in &entries {
        if e.inode_type == crate::vfs::InodeType::File && e.name.ends_with("_testcode.sh") {
            scripts.push((tests_root.to_string(), e.name.clone()));
        }
    }

    // 2) One-level deep: tests_root/*/*. This matches the official images layout:
    //    /tests/glibc/*.sh and /tests/musl/*.sh.
    for e in entries {
        if e.inode_type != crate::vfs::InodeType::Directory {
            continue;
        }
        let subdir = if tests_root == "/" {
            format!("/{}", e.name)
        } else {
            format!("{}/{}", tests_root.trim_end_matches('/'), e.name)
        };
        let Ok(d) = vfs_lookup(&subdir) else {
            continue;
        };
        let Ok(subents) = d.inode.readdir() else {
            continue;
        };
        for se in subents {
            if se.inode_type == crate::vfs::InodeType::File && se.name.ends_with("_testcode.sh") {
                scripts.push((subdir.clone(), se.name));
            }
        }
    }

    scripts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    scripts
}

fn parse_cmdline_u64(cmdline: &str, key: &str) -> Option<u64> {
    // Tokens are typically like: "foo=bar baz=1 quiet"
    for tok in cmdline.split_whitespace() {
        let Some(v) = tok.strip_prefix(key).and_then(|s| s.strip_prefix('=')) else {
            continue;
        };
        if v.is_empty() {
            continue;
        }
        if let Ok(n) = v.parse::<u64>() {
            return Some(n);
        }
    }
    None
}

fn oscomp_test_timeout_secs() -> usize {
    // Default: 60s. `0` disables timeout.
    let mut timeout = option_env!("OSCOMP_TEST_TIMEOUT_SECS")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60) as usize;

    let cmdline = CMDLINE.read();
    if !cmdline.is_empty() {
        // Accept a couple of aliases to make it easy to type.
        if let Some(v) = parse_cmdline_u64(&cmdline, "oscomp.test_timeout")
            .or_else(|| parse_cmdline_u64(&cmdline, "oscomp.timeout"))
        {
            timeout = v as usize;
        }
    }
    timeout
}

fn uts_bytes(s: &str) -> [u8; UTS_NAME_LEN] {
    let mut buf = [0u8; UTS_NAME_LEN];
    let bytes = s.as_bytes();
    let n = bytes.len().min(UTS_NAME_LEN - 1); // keep NUL-termination space
    buf[..n].copy_from_slice(&bytes[..n]);
    buf
}

/// Return a Linux-like `uname()` identity for OSCOMP test binaries.
///
/// Some official test binaries (notably glibc-built BusyBox) abort early if `uname().release`
/// looks "too old".
pub fn oscomp_uts_namespace() -> UtsNamespace {
    // Some glibc-based statically linked binaries in the official test image will refuse to run
    // if uname().release looks "too old". Present a Linux-like release string.
    //
    // Keep it *moderate* (not too new): some userlands may enable newer syscalls based on
    // kernel version parsing. The official test binaries are built for GNU/Linux 4.15.0.
    let mut uts = UtsNamespace::with_arch(crate::arch::constant::ARCH);
    uts.sysname = uts_bytes("Linux");
    uts.release = uts_bytes("4.15.0");
    uts.version = uts_bytes("SanktaOS 4.15.0");
    uts
}

fn spawn_user_and_wait(
    program: &str,
    argv: &[&str],
    envp: &[&str],
    cwd: &str,
    timeout_secs: usize,
) -> Result<i32, ()> {
    use crate::vfs::FDTable;
    use core::ptr::null_mut;
    use uapi::resource::{INIT_RLIMITS, RlimitStruct};
    use uapi::wait::WaitFlags;

    crate::earlyprintln!("[OSCOMP] spawn: preparing {}", program);
    let prepared = prepare_exec_image_from_path(program).map_err(|e| {
        crate::pr_warn!(
            "[OSCOMP] prepare_exec_image_from_path({}) failed: {:?}",
            program,
            e
        );
    })?;
    crate::earlyprintln!(
        "[OSCOMP] spawn: prepared {}, entry={:#x}",
        program,
        prepared.initial_pc
    );

    let space = alloc::sync::Arc::new(SpinLock::new(prepared.space));

    // Standard IO to console.
    let fd_table = FDTable::new();
    let (_stdin, stdout, stderr) = create_stdio_files();
    let _ = fd_table.install_at(0, alloc::sync::Arc::new(NullStdin));
    let _ = fd_table.install_at(1, stdout);
    let _ = fd_table.install_at(2, stderr);

    let root = get_root_dentry().ok();
    let cwd_dentry = vfs_lookup(cwd).ok().or_else(|| root.clone());
    let fs = alloc::sync::Arc::new(SpinLock::new(crate::kernel::FsStruct::new(
        cwd_dentry, root,
    )));

    let tid = TASK_MANAGER.lock().allocate_tid();
    let pid = tid;
    let ppid = { current_task().lock().pid };

    let kstack_tracker = alloc_contig_frames(4).ok_or(())?;
    let trap_frame_tracker = alloc_frame().ok_or(())?;

    let task = TaskStruct::utask_create(
        tid,
        pid,
        ppid,
        pid, // pgid
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        space.clone(),
        alloc::sync::Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        alloc::sync::Arc::new(SpinLock::new(SignalPending::empty())),
        alloc::sync::Arc::new(SpinLock::new(SignalStack::default())),
        0,
        alloc::sync::Arc::new(SpinLock::new(oscomp_uts_namespace())),
        alloc::sync::Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        alloc::sync::Arc::new(fd_table),
        fs,
    )
    .into_shared();

    // Establish parent/child relationship (best-effort; runner may also wait by polling).
    current_task().lock().children.lock().push(task.clone());

    // Build the userspace stack + trapframe for the child under its address space.
    crate::earlyprintln!("[OSCOMP] spawn: setting up child execve (pid={})", pid);
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(space.clone());
    }
    {
        let mut t = task.lock();
        t.exe_path = Some(program.to_string());
        t.execve(
            space,
            prepared.initial_pc,
            prepared.user_sp_high,
            argv,
            envp,
            prepared.phdr_addr,
            prepared.phnum,
            prepared.phent,
            prepared.at_base,
            prepared.at_entry,
        );
    }
    crate::earlyprintln!("[OSCOMP] spawn: child execve ready (pid={})", pid);
    // Switch back to the kernel/global space for the runner.
    {
        let kernel_space = crate::mm::get_global_kernel_space();
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    // Schedule the child.
    let target_cpu = pick_cpu();
    task.lock().on_cpu = Some(target_cpu);
    TASK_MANAGER.lock().add_task(task.clone());
    scheduler_of(target_cpu).lock().add_task(task);
    let cur_cpu = crate::arch::kernel::cpu::cpu_id();
    if target_cpu != cur_cpu {
        crate::arch::ipi::send_reschedule_ipi(target_cpu);
    }
    crate::earlyprintln!(
        "[OSCOMP] spawn: child scheduled (pid={}, cpu={})",
        pid,
        target_cpu
    );

    // Wait until the child exits, then reap it via the existing wait4 implementation.
    //
    // IMPORTANT: use WNOHANG + timeout to avoid hanging the judge forever on a buggy test.
    let deadline = if timeout_secs == 0 {
        None
    } else {
        let start = crate::arch::timer::get_time();
        Some(start.saturating_add(crate::arch::timer::clock_freq() * timeout_secs))
    };

    loop {
        let r = crate::kernel::syscall::wait4_kernel(
            pid as i32,
            null_mut(),
            WaitFlags::NOHANG.bits() as i32,
            null_mut(),
        );
        if r == pid as i32 {
            // Exit code is encoded in wstatus; we didn't request it. Best-effort read from task.
            // If the task is already reaped, default to 0.
            let code = TASK_MANAGER
                .lock()
                .get_task(tid)
                .and_then(|t| t.lock().exit_code)
                .unwrap_or(0);
            return Ok(code);
        }
        if r < 0 {
            crate::pr_warn!("[OSCOMP] wait4(pid={}) failed: {}", pid, r);
            return Ok(r);
        }

        // Not exited yet.
        if let Some(deadline) = deadline {
            if crate::arch::timer::get_time() >= deadline {
                crate::pr_warn!(
                    "[OSCOMP] timeout waiting for pid {} (program: {}); continuing",
                    pid,
                    program
                );
                return Ok(-1);
            }
        }
        yield_task();
    }
}

/// OSCOMP flow entry.
pub fn init() -> ! {
    crate::earlyprintln!("[OSCOMP] entering oscomp mode (kernel-runner)");

    setup_minimal_mounts();

    let timeout_secs = oscomp_test_timeout_secs();
    if timeout_secs == 0 {
        crate::earlyprintln!("[OSCOMP] per-test timeout: disabled");
    } else {
        crate::earlyprintln!("[OSCOMP] per-test timeout: {}s", timeout_secs);
    }

    let tests_root = pick_tests_root();
    let scripts = list_test_scripts(tests_root);
    crate::earlyprintln!(
        "[OSCOMP] discovered {} test scripts under {}",
        scripts.len(),
        tests_root
    );

    // Choose a shell from rootfs. Prefer BusyBox `ash` (more POSIX-like); fall back to `sh`.
    let shell = if vfs_lookup("/bin/ash").is_ok() {
        "/bin/ash"
    } else {
        "/bin/sh"
    };
    if vfs_lookup(shell).is_err() {
        crate::earlyprintln!("[OSCOMP] no usable shell found; cannot run test scripts");
        shutdown(true)
    }

    // Use a minimal env; scripts usually rely on PATH.
    let envp = ["PATH=/bin:/sbin:/usr/bin:/usr/sbin:/tests", "HOME=/"];

    for (dir, name) in scripts {
        // Use "-c" to force a known working directory even if initial cwd setup fails in the FS layer.
        // Script bodies rely heavily on relative paths like "./busybox" and "cd ./basic".
        // Run the script via shell builtin "." to avoid relying on kernel shebang/ENOEXEC behavior.
        let cmd = format!("cd {} && . ./{}", dir, name);
        crate::earlyprintln!("[OSCOMP] running {}/{}", dir, name);
        let _ = spawn_user_and_wait(shell, &[shell, "-c", &cmd], &envp, "/", timeout_secs);
    }

    crate::earlyprintln!("[OSCOMP] all tests finished; shutting down");
    shutdown(false)
}
