//! Tmpfs 文件系统实现

use alloc::sync::Arc;

use crate::ops::fs_ops;
use sync::SpinLock;
use vfs::{FileSystem, FsError, Inode, StatFs};

use super::inode::{TmpfsInode, TmpfsStats};

/// Tmpfs 文件系统
pub struct TmpFs {
    /// 根 inode
    root: Arc<TmpfsInode>,

    /// 统计信息
    stats: Arc<SpinLock<TmpfsStats>>,
}

impl TmpFs {
    /// 创建新的 tmpfs 文件系统
    ///
    /// # 参数
    ///
    /// - `max_size_mb`: 最大容量（MB），0 表示无限制
    pub fn new(max_size_mb: usize) -> Arc<Self> {
        let page_size = fs_ops().page_size();
        let max_pages = if max_size_mb == 0 {
            0 // 无限制
        } else {
            max_size_mb * 1024 * 1024 / page_size
        };

        let stats = Arc::new(SpinLock::new(TmpfsStats {
            allocated_pages: 0,
            max_pages,
            next_inode_no: 1,
        }));

        let root = TmpfsInode::new_root(stats.clone());

        Arc::new(Self { root, stats })
    }

    /// 获取已使用的容量（字节）
    pub fn used_size(&self) -> usize {
        let stats = self.stats.lock();
        stats.allocated_pages * fs_ops().page_size()
    }

    /// 获取总容量（字节，0 表示无限制）
    pub fn total_size(&self) -> usize {
        let stats = self.stats.lock();
        if stats.max_pages == 0 {
            0
        } else {
            stats.max_pages * fs_ops().page_size()
        }
    }
}

impl FileSystem for TmpFs {
    fn fs_type(&self) -> &'static str {
        "tmpfs"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root.clone() as Arc<dyn Inode>
    }

    fn sync(&self) -> Result<(), FsError> {
        // tmpfs 完全在内存中，无需同步
        Ok(())
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        let stats = self.stats.lock();
        let page_size = fs_ops().page_size();

        let total_blocks = if stats.max_pages == 0 {
            // 无限制时，使用一个较大的值
            usize::MAX / page_size
        } else {
            stats.max_pages
        };

        let free_blocks = if stats.max_pages == 0 {
            total_blocks
        } else {
            stats.max_pages.saturating_sub(stats.allocated_pages)
        };

        Ok(StatFs {
            block_size: page_size,
            total_blocks,
            free_blocks,
            available_blocks: free_blocks,
            total_inodes: 0, // tmpfs 动态分配，无上限
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }

    fn umount(&self) -> Result<(), FsError> {
        // tmpfs 卸载时自动释放所有内存（通过 Drop）
        Ok(())
    }
}
