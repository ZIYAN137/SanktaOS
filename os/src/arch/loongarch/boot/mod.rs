//! LoongArch64 架构相关的启动代码

use core::{
    arch::{asm, global_asm},
    sync::atomic::Ordering,
};

use alloc::sync::Arc;

use crate::{
    arch::{
        intr,
        mm::{paddr_to_vaddr, vaddr_to_paddr},
        platform, timer, trap,
    },
    earlyprintln,
    ipc::{SignalHandlerTable, SignalPending},
    kernel::{
        FsStruct, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu, current_task,
        kernel_execve, kthread_spawn, kworker, sleep_task_with_block, time, yield_task,
    },
    mm::{
        self,
        frame_allocator::{alloc_contig_frames, alloc_frame},
    },
    pr_err, pr_info, println,
    sync::SpinLock,
    uapi::{
        resource::{INIT_RLIMITS, RlimitStruct},
        signal::SignalFlags,
        uts_namespace::UtsNamespace,
    },
    vfs::{FDTable, create_stdio_files, get_root_dentry},
};

global_asm!(include_str!("entry.S"));

/// 内核的第一个任务启动函数
/// 并且当这个函数结束时，应该切换到第一个任务的上下文
pub fn rest_init() {
    earlyprintln!("[Boot] rest_init: creating init task");
    // init 进程必须使用 TID/PID 1，不从分配器获取（分配器从 2 开始）。
    let tid = 1;
    let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");
    let fd_table = FDTable::new();
    let (stdin, stdout, stderr) = create_stdio_files();
    fd_table
        .install_at(0, stdin)
        .expect("Failed to install stdin");
    fd_table
        .install_at(1, stdout)
        .expect("Failed to install stdout");
    fd_table
        .install_at(2, stderr)
        .expect("Failed to install stderr");
    let cwd = get_root_dentry().ok();
    let root = cwd.clone();
    let fs = Arc::new(SpinLock::new(FsStruct::new(cwd, root)));

    // In OSCOMP mode, present a Linux-like uname() to satisfy official test binaries
    // (some abort with "FATAL: kernel too old" if release is not >= 4.15.0).
    let uts = {
        #[cfg(feature = "oscomp")]
        {
            crate::oscomp::oscomp_uts_namespace()
        }
        #[cfg(not(feature = "oscomp"))]
        {
            UtsNamespace::with_arch(crate::arch::constant::ARCH)
        }
    };

    let mut task = TaskStruct::ktask_create(
        tid,
        tid,
        0,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        Arc::new(SpinLock::new(uts)),
        Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        Arc::new(fd_table),
        fs,
    ); // init 没有父任务

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    // Safety: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
    unsafe {
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(init as usize, 0, task.kstack_base);
    }

    let ra = task.context.ra;
    let sp = task.context.sp;
    let ptr = task.trap_frame_ptr.load(Ordering::SeqCst);
    // init 任务运行在 CPU 0
    task.on_cpu = Some(0);
    let task = task.into_shared();

    // 为 CPU0 创建 idle 任务，避免调度器在 runqueue 为空时 panic。
    // idle 任务不加入运行队列，但会作为兜底任务被切换运行。
    {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
        if cpu.idle_task.is_none() {
            cpu.idle_task = Some(create_idle_task(0));
        }
    }

    unsafe {
        // KScratch0 <- TrapFrame 指针
        asm!("csrwr {0}, 0x30", in(reg) ptr as usize, options(nostack, preserves_flags));
    }
    TASK_MANAGER.lock().add_task(task.clone());
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_task(task);
    }

    earlyprintln!("[Boot] rest_init: switching to init");

    // 切入 kinit：设置 sp 并跳到 ra；此调用不返回
    // SAFETY: 在 Task 创建时已正确初始化 ra 和 sp
    unsafe {
        asm!(
            "addi.d $sp, {sp}, 0",
            "jirl $zero, {ra}, 0",
            sp = in(reg) sp,
            ra = in(reg) ra,
            options(noreturn)
        );
    }
}

/// Idle 循环：等待中断；被定时器中断唤醒后由 trap/scheduler 决定是否调度。
fn idle_loop() -> ! {
    loop {
        if !crate::arch::intr::are_interrupts_enabled() {
            unsafe { crate::arch::intr::enable_interrupts() };
        }
        unsafe {
            core::arch::asm!("idle 0");
        }
    }
}

/// 为指定 CPU 创建 idle 任务（LoongArch 版本）
fn create_idle_task(cpu_id: usize) -> crate::kernel::SharedTask {
    use crate::arch::trap::TrapFrame;
    use crate::vfs::FDTable;
    use mm::frame_allocator::alloc_contig_frames;

    // idle 任务从 TID 分配器正常分配（从 2 开始）
    let tid = TASK_MANAGER.lock().allocate_tid();

    // 分配最小资源
    let kstack_tracker =
        alloc_contig_frames(1).expect("Failed to allocate kernel stack for idle task");
    let trap_frame_tracker = alloc_frame().expect("Failed to allocate trap frame for idle task");

    // 创建最小化的内核线程
    let mut task = TaskStruct::ktask_create(
        tid,
        tid, // pid = tid
        0,   // ppid = 0 (no parent)
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        Arc::new(SpinLock::new(UtsNamespace::with_arch(
            crate::arch::constant::ARCH,
        ))),
        Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        Arc::new(FDTable::new()),
        Arc::new(SpinLock::new(FsStruct::new(None, None))),
    );

    // 设置 TrapFrame 指向 idle_loop
    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(idle_loop as usize, 0, task.kstack_base);
    }

    task.on_cpu = Some(cpu_id);
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());
    task
}

/// 内核的第一个任务
/// PID = 1
/// 负责进行剩余的初始化工作
/// 创建 kthreadd 任务
/// 并在一切结束后转化为第一个用户态任务
fn init() {
    earlyprintln!("[Init] entered init()");
    super::trap::init();

    // 启用中断（在设置好 trap 处理与 KScratch0 之后）
    unsafe { intr::enable_interrupts() };

    create_kthreadd();

    #[cfg(feature = "oscomp")]
    {
        if let Err(e) = crate::fs::init_oscomp_filesystems() {
            pr_err!(
                "[Init][OSCOMP] Warning: Failed to initialize filesystems: {:?}",
                e
            );
            pr_info!("[Init][OSCOMP] Continuing without filesystem...");
        }
    }
    #[cfg(not(feature = "oscomp"))]
    {
        // 初始化 Ext4 文件系统（从真实块设备）
        // 必须在任务上下文中进行,因为 VFS 需要 current_task()
        if let Err(e) = crate::fs::init_ext4_from_block_device() {
            pr_err!(
                "[Init] Warning: Failed to initialize Ext4 filesystem: {:?}",
                e
            );
            pr_info!("[Init] Continuing without filesystem...");
        }
    }

    // /dev(/proc,/sys,/tmp) 的挂载交给用户态 rcS：
    // - rcS 会执行 `mount -t tmpfs none /dev` 等
    // - 内核在 mount("/dev") 的系统调用里会自动 init_dev() 创建设备节点
    // Always enter user-space BusyBox init. In OSCOMP mode, /tests is mounted by
    // init_oscomp_filesystems() and rcS is responsible for running the test scripts.
    kernel_execve("/sbin/init", &["/sbin/init"], &[]);
}

/// 内核守护线程
/// PID = 2
/// 负责创建内核任务，回收僵尸任务等工作
fn kthreadd() {
    kthread_spawn(kworker);
    loop {
        // 休眠等待任务
        sleep_task_with_block(current_task(), true);
        yield_task();
    }
}

/// 创建内核守护线程 kthreadd
fn create_kthreadd() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");
    let (uts, rlimit, fd_table, fs) = {
        let task = current_task();
        let t = task.lock();
        (
            t.uts_namespace.clone(),
            t.rlimit.clone(),
            t.fd_table.clone_table(),
            t.fs.lock().clone(),
        )
    };
    let task = TaskStruct::ktask_create(
        tid,
        tid,
        0,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        uts,
        rlimit,
        Arc::new(fd_table),
        Arc::new(SpinLock::new(fs)),
    ); // kthreadd 没有父任务

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    // Safety: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
    unsafe {
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(kthreadd as usize, 0, task.kstack_base);
    }
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());
    task.lock().on_cpu = Some(0);
    crate::kernel::scheduler_of(0).lock().add_task(task);
}

pub fn main(hartid: usize) {
    clear_bss();

    // Enable base floating-point instructions (EUEN.FPE). Many LoongArch Linux-ABI
    // user programs are built with floating-point enabled and may execute FP
    // instructions very early during startup.
    loongArch64::register::euen::set_fpe(true);

    // 初始化 sync crate 的架构操作（必须在任何使用 sync 原语之前）
    unsafe { crate::arch::init_sync_arch_ops() };

    earlyprintln!("[Boot] Hello, world!");
    earlyprintln!("[Boot] LoongArch CPU {} is up!", hartid);

    // 注册 mm crate 的配置和架构操作（必须在 phase1_early_parse() 之前，因为需要地址转换）
    unsafe {
        crate::config::register_mm_config();
        crate::arch::mm::register_mm_ops();
    }

    // ========== Phase 1: 早期设备树解析（无堆分配）==========
    unsafe {
        crate::device::device_tree::phase1_early_parse();
    }

    // 从 Phase 1 数据设置 NUM_CPU 和 CLOCK_FREQ
    unsafe {
        crate::kernel::NUM_CPU = crate::device::device_tree::early_num_cpus();
        crate::kernel::CLOCK_FREQ = crate::device::device_tree::early_clock_freq();
    }

    earlyprintln!(
        "[Boot] Early DT: {} CPU(s), {} Hz",
        unsafe { crate::kernel::NUM_CPU },
        unsafe { crate::kernel::CLOCK_FREQ }
    );

    // Initialize MM subsystem (heap, frame allocator, kernel page tables).
    mm::init();

    // Activate kernel address space and set current_memory_space (needed by rest_init/current_memory_space).
    {
        let kernel_space = crate::mm::get_global_kernel_space();
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
    }

    // 测试模式下：
    // - 提前注册 FsOps，避免 tests/ 中的 TmpFs/Ext4 等依赖 fs_ops() 时 panic。
    // - 提前创建并切换到一个可用的 current_task，避免 /proc/self 等依赖 current_task() 的用例直接 panic。
    #[cfg(test)]
    {
        crate::fs::init_fs_ops();
        crate::vfs::init_vfs_ops();

        let _guard = crate::sync::PreemptGuard::new();
        if current_cpu().current_task.is_none() {
            let idle0 = create_idle_task(0);
            let tf_ptr = idle0.lock().trap_frame_ptr.load(Ordering::SeqCst);
            unsafe {
                // KScratch0 <- TrapFrame 指针（trap/restore 路径会使用）
                asm!(
                    "csrwr {0}, 0x30",
                    in(reg) tf_ptr as usize,
                    options(nostack, preserves_flags)
                );
            }

            let cpu = current_cpu();
            cpu.idle_task = Some(idle0.clone());
            cpu.switch_task(idle0);
        }

        crate::test_main();
    }

    // 初始化工作
    trap::init_boot_trap();

    // 初始化设备操作（必须在 platform::init() 之前，因为设备树初始化会注册中断）
    crate::device::init_device_ops();

    platform::init();
    time::init();
    earlyprintln!("[Boot] time::init finished");
    timer::init();
    earlyprintln!("[Boot] timer::init finished");

    // 初始化 VFS 操作（必须在使用 VFS 之前）
    crate::vfs::init_vfs_ops();

    // 初始化 FS 操作（必须在使用 fs crate 之前）
    #[cfg(not(test))]
    crate::fs::init_fs_ops();

    earlyprintln!("[Boot] entering rest_init");
    rest_init();
}

/// 清除 BSS 段，将其全部置零
/// BSS 段包含所有未初始化的静态变量
/// 在进入 Rust 代码之前调用此函数非常重要
fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    let sbss_paddr = unsafe { vaddr_to_paddr(sbss as usize) };
    let ebss_paddr = unsafe { vaddr_to_paddr(ebss as usize) };

    (sbss_paddr..ebss_paddr).for_each(|a| unsafe {
        let va = paddr_to_vaddr(a);
        (va as *mut u8).write_volatile(0)
    });
}
