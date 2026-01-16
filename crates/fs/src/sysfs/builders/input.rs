//! 输入设备 sysfs 树构建器

use alloc::format;
use alloc::sync::Arc;

use vfs::{FsError, Inode};

use crate::sysfs::device_registry;
use crate::sysfs::inode::SysfsInode;

/// 构建输入设备 sysfs 树
pub fn build_input_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    let class_inode = root.lookup("class")?;
    let class = class_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    let input_inode = class.lookup("input")?;
    let input_dir = input_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    for dev_info in device_registry::list_input_devices() {
        let target = format!("../../devices/platform/{}", dev_info.name);
        let symlink = SysfsInode::new_symlink(target);
        input_dir.add_child(&dev_info.name, symlink)?;
    }

    Ok(())
}
