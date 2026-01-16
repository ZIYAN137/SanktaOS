//! TTY 设备 sysfs 树构建器

use alloc::format;
use alloc::sync::Arc;

use vfs::{FsError, Inode};

use crate::sysfs::device_registry;
use crate::sysfs::inode::SysfsInode;

/// 构建 TTY 设备 sysfs 树
pub fn build_tty_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let tty_inode = class.lookup("tty")?;
    let tty_dir = tty_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    for dev_info in device_registry::list_tty_devices() {
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        tty_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
