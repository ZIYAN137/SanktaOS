//! VFS 与 mm crate 的桥接模块
//!
//! 为 os crate 的 Inode 和 File trait 实现 mm crate 的 MmInode 和 MmFile trait，
//! 使得 VFS 文件可以用于内存映射。

use alloc::sync::Arc;
use mm::{MmFile, MmInode};

use super::{File, Inode};

/// Inode 的包装类型，实现 MmInode trait
pub struct InodeWrapper(pub Arc<dyn Inode>);

impl MmInode for InodeWrapper {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, isize> {
        self.0.read_at(offset, buf).map_err(|e| e.to_errno())
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, isize> {
        self.0.write_at(offset, buf).map_err(|e| e.to_errno())
    }
}

/// File 的包装类型，实现 MmFile trait
pub struct FileWrapper(pub Arc<dyn File>);

impl MmFile for FileWrapper {
    fn inode(&self) -> Result<Arc<dyn MmInode>, isize> {
        self.0
            .inode()
            .map(|inode| Arc::new(InodeWrapper(inode)) as Arc<dyn MmInode>)
            .map_err(|e| e.to_errno())
    }
}
