//! 文件抽象层 - VFS 会话层接口
//!
//! 该模块定义了统一的文件操作接口 [`File`] trait，支持普通文件、管道、字符设备等多种文件类型。
//! 所有打开的文件以 `Arc<dyn File>` 形式存储在进程的文件描述符表中。
//!
//! 与 [`crate::Inode`] 的区别：
//!
//! - `File` 通常是“有状态”的（例如维护当前 offset），适合实现 `read/write/lseek` 语义。
//! - `Inode` 更偏“无状态存储接口”，用于提供底层随机访问与元数据操作。

use alloc::sync::Arc;
use uapi::fcntl::{OpenFlags, SeekWhence};

use crate::{Dentry, FsError, Inode, InodeMetadata};

/// 文件操作的统一接口
///
/// 所有打开的文件以 `Arc<dyn File>` 形式存储在进程的文件描述符表中。
pub trait File: Send + Sync {
    /// 检查文件是否可读
    fn readable(&self) -> bool;

    /// 检查文件是否可写
    fn writable(&self) -> bool;

    /// 从文件读取数据
    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向文件写入数据
    fn write(&self, buf: &[u8]) -> Result<usize, FsError>;

    /// 获取文件元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 设置文件偏移量（可选方法）
    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取当前偏移量（可选方法）
    fn offset(&self) -> usize {
        0
    }

    /// 获取打开标志（可选方法）
    fn flags(&self) -> OpenFlags {
        OpenFlags::empty()
    }

    /// 获取目录项（可选方法）
    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取Inode（可选方法）
    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置文件状态标志（可选方法，用于 F_SETFL）
    fn set_status_flags(&self, _flags: OpenFlags) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取管道大小（可选方法，用于 F_GETPIPE_SZ）
    fn get_pipe_size(&self) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置管道大小（可选方法，用于 F_SETPIPE_SZ）
    fn set_pipe_size(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取异步 I/O 所有者（可选方法，用于 F_GETOWN）
    fn get_owner(&self) -> Result<i32, FsError> {
        Err(FsError::NotSupported)
    }

    /// 设置异步 I/O 所有者（可选方法，用于 F_SETOWN）
    fn set_owner(&self, _pid: i32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    /// 从指定位置读取数据（可选方法，用于 pread64/preadv）
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 向指定位置写入数据（可选方法，用于 pwrite64/pwritev）
    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 执行设备特定的控制操作（可选方法，用于 ioctl）
    fn ioctl(&self, _request: u32, _arg: usize) -> Result<isize, FsError> {
        Err(FsError::NotSupported)
    }

    /// 获取 Any trait 引用，用于安全的类型转换
    fn as_any(&self) -> &dyn core::any::Any;

    /// 从socket接收数据并获取源地址（可选方法，用于recvfrom）
    fn recvfrom(&self, _buf: &mut [u8]) -> Result<(usize, Option<alloc::vec::Vec<u8>>), FsError> {
        Err(FsError::NotSupported)
    }
}
