//! 虚拟文件系统（VFS）层
//!
//! 此模块重新导出 vfs crate 的所有公共接口，并提供 os crate 特定的实现。

mod mm_bridge;
mod ops_impl;

// Re-export everything from vfs crate
pub use vfs::*;

// Re-export mm_bridge
pub use mm_bridge::{FileWrapper, InodeWrapper};

// Re-export ops_impl for initialization
pub use ops_impl::init_vfs_ops;

use alloc::{vec, vec::Vec};

/// 从指定路径加载 ELF 文件内容
///
/// 参数：
///     - path: 文件路径（绝对路径或相对于当前工作目录的相对路径）
///
/// 返回：`Ok(Vec<u8>)` 文件内容字节数组；`Err(FsError::NotFound)` 文件不存在；`Err(FsError::IsDirectory)` 路径指向目录
pub fn vfs_load_elf(path: &str) -> Result<Vec<u8>, FsError> {
    let dentry = vfs_lookup(path)?;
    let inode = &dentry.inode;
    let metadata = inode.metadata()?;

    // 确保是普通文件
    if metadata.inode_type != InodeType::File {
        return Err(FsError::IsDirectory);
    }

    let mut buf = vec![0u8; metadata.size];
    inode.read_at(0, &mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests;
