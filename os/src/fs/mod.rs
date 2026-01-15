//! # 文件系统模块 (FS)
//!
//! 本模块 re-export fs crate 的内容，并提供初始化函数。

mod ops_impl;

// Re-export fs crate (使用 :: 前缀引用外部 crate，避免与本模块名冲突)
pub use ::fs::*;

use alloc::string::String;

use crate::device::BLK_DRIVERS;
use crate::pr_info;
use crate::vfs::{blkdev_major, chrdev_major, makedev};
use crate::vfs::{FileMode, FsError, MountFlags, MOUNT_TABLE, vfs_lookup};

/// 初始化 FS 操作实现
pub fn init_fs_ops() {
    ops_impl::init();
}

/// 从真实的块设备初始化 Ext4 文件系统
pub fn init_ext4_from_block_device() -> Result<(), FsError> {
    use crate::config::{EXT4_BLOCK_SIZE, FS_IMAGE_SIZE};

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
    let total_blocks = FS_IMAGE_SIZE / ext4_block_size;

    pr_info!(
        "[Ext4] Ext4 block size: {}, Total blocks: {}, Image size: {} MB",
        ext4_block_size,
        total_blocks,
        FS_IMAGE_SIZE / 1024 / 1024
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
