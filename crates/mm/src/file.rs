//! 文件映射接口 trait 定义

use alloc::sync::Arc;

/// 可用于内存映射读写的 Inode 接口
///
/// 此 trait 抽象了文件 I/O 所需的最小接口。
pub trait MmInode: Send + Sync {
    /// 从指定偏移读取数据到缓冲区
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, isize>;

    /// 将缓冲区数据写入指定偏移
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, isize>;
}

/// 可映射到内存的文件接口
///
/// 此 trait 抽象了文件映射所需的最小接口。
/// vfs::File 需要实现此 trait。
pub trait MmFile: Send + Sync {
    /// 获取底层 Inode 用于读写操作
    fn inode(&self) -> Result<Arc<dyn MmInode>, isize>;
}
