//! # 文件系统模块 (FS)
//!
//! 本模块 re-export fs crate 的内容，并提供初始化函数。

mod ops_impl;

// Re-export fs crate (使用 :: 前缀引用外部 crate，避免与本模块名冲突)
pub use ::fs::*;

use alloc::string::String;

use crate::device::BLK_DRIVERS;
use crate::pr_info;
use crate::vfs::{FileMode, FsError, MOUNT_TABLE, MountFlags, vfs_lookup};
use crate::vfs::{blkdev_major, chrdev_major, makedev};

/// 初始化 FS 操作实现
pub fn init_fs_ops() {
    ops_impl::init();
}

fn ensure_top_level_dir(path: &str, mode: FileMode) -> Result<(), FsError> {
    // Only used for simple mount points like "/dev" "/tests" etc.
    if vfs_lookup(path).is_ok() {
        return Ok(());
    }
    let name = path.strip_prefix('/').ok_or(FsError::InvalidArgument)?;
    if name.is_empty() || name.contains('/') {
        return Err(FsError::InvalidArgument);
    }
    let root = crate::vfs::get_root_dentry()?;
    root.inode.mkdir(name, mode)?;
    Ok(())
}

fn set_current_task_root_cwd_to_vfs_root() -> Result<(), FsError> {
    let root = crate::vfs::get_root_dentry()?;
    let task = crate::kernel::current_task();
    let task_guard = task.lock();
    let mut fs = task_guard.fs.lock();
    fs.root = Some(root.clone());
    fs.cwd = Some(root);
    Ok(())
}

/// 从真实的块设备初始化 Ext4 文件系统
pub fn init_ext4_from_block_device() -> Result<(), FsError> {
    use crate::config::EXT4_BLOCK_SIZE;

    pr_info!("[Ext4] Initializing Ext4 filesystem from block device");

    let blk_drivers = BLK_DRIVERS.read();
    if blk_drivers.is_empty() {
        pr_info!("[Ext4] No block device found");
        return Err(FsError::NoDevice);
    }

    let block_driver = blk_drivers[0].clone();
    drop(blk_drivers);

    pr_info!("[Ext4] Using block device: {}", block_driver.get_id());

    let ext4_block_size = EXT4_BLOCK_SIZE;
    let device_bytes = block_driver
        .total_blocks()
        .saturating_mul(block_driver.block_size());
    let total_blocks = device_bytes / ext4_block_size;

    pr_info!(
        "[Ext4] Ext4 block size: {}, Total blocks: {}, Device size: {} MB",
        ext4_block_size,
        total_blocks,
        device_bytes / 1024 / 1024
    );

    let ext4_fs = Ext4FileSystem::open(block_driver, ext4_block_size, total_blocks, 0)?;

    pr_info!("[Ext4] Mounting Ext4 as root filesystem");
    MOUNT_TABLE.mount(
        ext4_fs,
        "/",
        MountFlags::empty(),
        Some(String::from("virtio-blk0")),
    )?;

    pr_info!("[Ext4] Root filesystem mounted at /");

    // Keep VFS ops (current task cwd/root) consistent with the new root mount.
    let _ = set_current_task_root_cwd_to_vfs_root();

    if let Ok(root_dentry) = crate::vfs::get_root_dentry() {
        pr_info!("[Ext4] Root directory contents:");
        let inode = root_dentry.inode.clone();
        if let Ok(entries) = inode.readdir() {
            for entry in entries {
                pr_info!("  - {} (type: {:?})", entry.name, entry.inode_type);
            }
        } else {
            pr_info!("[Ext4] Failed to read root directory");
        }
    }

    Ok(())
}

/// OSCOMP: mount rootfs from the second disk (x1) and the judge-provided test disk (x0) at /tests.
///
/// QEMU (judge) wires:
/// - x0: `{fs}` test image (ext4, contains `*_testcode.sh`)
/// - x1: our `disk.img` / `disk-la.img` (ext4 rootfs, contains `/bin/sh`)
pub fn init_oscomp_filesystems() -> Result<(), FsError> {
    use crate::config::EXT4_BLOCK_SIZE;

    pr_info!("[OSCOMP][Ext4] Initializing filesystems (rootfs + testfs)");

    // The probe order of virtio-blk devices is not stable across QEMU setups.
    // Identify disks by content:
    // - rootfs must contain `/bin/sh`
    // - testfs contains `*_testcode.sh` at its root (mounted at /tests)
    let blk_list = BLK_DRIVERS.read();
    if blk_list.is_empty() {
        pr_info!("[OSCOMP][Ext4] No block device found");
        return Err(FsError::NoDevice);
    }
    let devices: alloc::vec::Vec<_> = blk_list.iter().cloned().collect();
    drop(blk_list);

    // 1) Probe + mount rootfs at "/" by checking `/bin/sh`.
    let mut root_idx: Option<usize> = None;
    for (idx, dev) in devices.iter().enumerate() {
        let bytes = dev.total_blocks().saturating_mul(dev.block_size());
        let total_blocks = bytes / EXT4_BLOCK_SIZE;
        let Ok(fs) = Ext4FileSystem::open(dev.clone(), EXT4_BLOCK_SIZE, total_blocks, idx) else {
            continue;
        };
        MOUNT_TABLE.mount(
            fs,
            "/",
            MountFlags::empty(),
            Some(alloc::format!("virtio-blk{}", idx)),
        )?;
        set_current_task_root_cwd_to_vfs_root()?;
        if crate::vfs::vfs_lookup("/bin/sh").is_ok() || crate::vfs::vfs_lookup("/bin/ash").is_ok() {
            pr_info!(
                "[OSCOMP][Ext4] Selected rootfs: virtio-blk{} (device_bytes={} MB)",
                idx,
                bytes / 1024 / 1024
            );
            root_idx = Some(idx);
            break;
        }
    }
    let root_idx = root_idx.ok_or(FsError::NoDevice)?;

    // Ensure common mount points exist on rootfs.
    let dir_mode = FileMode::S_IFDIR | FileMode::from_bits_truncate(0o755);
    let _ = ensure_top_level_dir("/dev", dir_mode);
    let _ = ensure_top_level_dir("/proc", dir_mode);
    let _ = ensure_top_level_dir("/sys", dir_mode);
    let _ = ensure_top_level_dir("/tmp", dir_mode);
    let _ = ensure_top_level_dir("/tests", dir_mode);

    fn testsuite_has_scripts(mount_root: &str) -> bool {
        // The official 2025 test image places scripts under `glibc/` and `musl/` rather than
        // directly in the filesystem root. Detect scripts at:
        //   - mount_root/*
        //   - mount_root/*/*   (one-level deep)
        let Ok(root) = crate::vfs::vfs_lookup(mount_root) else {
            return false;
        };
        let Ok(ents) = root.inode.readdir() else {
            return false;
        };
        if ents.iter().any(|e| e.name.ends_with("_testcode.sh")) {
            return true;
        }
        for e in ents {
            if e.inode_type != crate::vfs::InodeType::Directory {
                continue;
            }
            let sub = alloc::format!("{}/{}", mount_root.trim_end_matches('/'), e.name);
            let Ok(d) = crate::vfs::vfs_lookup(&sub) else {
                continue;
            };
            let Ok(sub_ents) = d.inode.readdir() else {
                continue;
            };
            if sub_ents.iter().any(|se| se.name.ends_with("_testcode.sh")) {
                return true;
            }
        }
        false
    }

    // 2) Probe + mount testfs at "/tests" by checking for `*_testcode.sh`.
    let mut test_found = false;
    for (idx, dev) in devices.iter().enumerate() {
        if idx == root_idx {
            continue;
        }
        let bytes = dev.total_blocks().saturating_mul(dev.block_size());
        let total_blocks = bytes / EXT4_BLOCK_SIZE;
        let Ok(fs) = Ext4FileSystem::open(dev.clone(), EXT4_BLOCK_SIZE, total_blocks, idx) else {
            continue;
        };
        MOUNT_TABLE.mount(
            fs,
            "/tests",
            MountFlags::empty(),
            Some(alloc::format!("virtio-blk{}", idx)),
        )?;

        // Check /tests for scripts (root or one-level deep).
        let ok = testsuite_has_scripts("/tests");
        if ok {
            pr_info!(
                "[OSCOMP][Ext4] Selected testfs: virtio-blk{} (device_bytes={} MB)",
                idx,
                bytes / 1024 / 1024
            );
            test_found = true;
            break;
        }
    }
    if !test_found {
        pr_info!("[OSCOMP][Ext4] Testfs not found; will scan / for test scripts");
    }

    Ok(())
}

/// 挂载 tmpfs 到指定路径
pub fn mount_tmpfs(mount_point: &str, max_size_mb: usize) -> Result<(), FsError> {
    use alloc::string::ToString;

    pr_info!(
        "[Tmpfs] Creating tmpfs filesystem (max_size: {} MB)",
        if max_size_mb == 0 {
            "unlimited".to_string()
        } else {
            max_size_mb.to_string()
        }
    );

    let tmpfs = TmpFs::new(max_size_mb);

    MOUNT_TABLE.mount(
        tmpfs,
        mount_point,
        MountFlags::empty(),
        Some(String::from("tmpfs")),
    )?;

    pr_info!("[Tmpfs] Tmpfs mounted at {}", mount_point);

    Ok(())
}

/// 初始化 /dev 目录下的设备文件
pub fn init_dev() -> Result<(), FsError> {
    if let Err(e) = vfs_lookup("/dev") {
        return Err(e);
    }

    create_devices()?;

    Ok(())
}

fn create_devices() -> Result<(), FsError> {
    let dev_dentry = vfs_lookup("/dev")?;
    let dev_inode = &dev_dentry.inode;

    let char_mode = FileMode::S_IFCHR | FileMode::from_bits_truncate(0o666);

    dev_inode.mknod("null", char_mode, makedev(chrdev_major::MEM, 3))?;
    dev_inode.mknod("zero", char_mode, makedev(chrdev_major::MEM, 5))?;
    dev_inode.mknod("random", char_mode, makedev(chrdev_major::MEM, 8))?;
    dev_inode.mknod("urandom", char_mode, makedev(chrdev_major::MEM, 9))?;

    let console_mode = FileMode::S_IFCHR | FileMode::from_bits_truncate(0o600);
    dev_inode.mknod("tty", console_mode, makedev(chrdev_major::CONSOLE, 0))?;
    dev_inode.mknod("console", console_mode, makedev(chrdev_major::CONSOLE, 1))?;

    dev_inode.mknod("ttyS0", char_mode, makedev(chrdev_major::TTY, 64))?;

    let dir_mode = FileMode::S_IFDIR | FileMode::from_bits_truncate(0o755);
    dev_inode.mkdir("misc", dir_mode)?;

    let misc_dentry = vfs_lookup("/dev/misc")?;
    misc_dentry
        .inode
        .mknod("rtc", char_mode, makedev(chrdev_major::MISC, 135))?;

    let block_mode = FileMode::S_IFBLK | FileMode::from_bits_truncate(0o660);
    dev_inode.mknod("vda", block_mode, makedev(blkdev_major::VIRTIO_BLK, 0))?;

    Ok(())
}

/// 初始化并挂载 procfs 到 /proc
pub fn init_procfs() -> Result<(), FsError> {
    pr_info!("[ProcFS] Initializing procfs");

    let procfs = ProcFS::new();
    procfs.init_tree()?;

    MOUNT_TABLE.mount(
        procfs,
        "/proc",
        MountFlags::empty(),
        Some(String::from("proc")),
    )?;

    pr_info!("[ProcFS] Procfs mounted at /proc");

    Ok(())
}

/// 初始化并挂载 sysfs 到 /sys
pub fn init_sysfs() -> Result<(), FsError> {
    pr_info!("[SysFS] Initializing sysfs");

    let sysfs = SysFS::new();
    sysfs.init_tree()?;

    MOUNT_TABLE.mount(
        sysfs,
        "/sys",
        MountFlags::empty(),
        Some(String::from("sysfs")),
    )?;

    pr_info!("[SysFS] Sysfs mounted at /sys");

    Ok(())
}

#[cfg(test)]
mod tests;
