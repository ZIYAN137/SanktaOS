//! Inode 抽象层 - VFS 存储层接口
//!
//! 该模块定义了文件系统的底层存储接口，提供无状态的文件和目录访问能力。
//!
//! `Inode` 侧接口通常以“显式 offset”的随机访问为主，因此可以被多个 [`crate::File`]
//! 会话对象共享；而 `File` 会话层负责维护 offset、flags 等打开状态。

use alloc::string::String;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use core::any::Any;
use uapi::time::TimeSpec;

use crate::{Dentry, FsError};

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    /// 普通文件
    File,
    /// 目录
    Directory,
    /// 符号链接
    Symlink,
    /// 字符设备
    CharDevice,
    /// 块设备
    BlockDevice,
    /// 命名管道
    Fifo,
    /// 套接字
    Socket,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    /// 文件权限和类型（与 POSIX 兼容）
    pub struct FileMode: u32 {
        // 文件类型掩码
        /// 文件类型掩码
        const S_IFMT   = 0o170000;
        /// 普通文件
        const S_IFREG  = 0o100000;
        /// 目录
        const S_IFDIR  = 0o040000;
        /// 符号链接
        const S_IFLNK  = 0o120000;
        /// 字符设备
        const S_IFCHR  = 0o020000;
        /// 块设备
        const S_IFBLK  = 0o060000;
        /// FIFO
        const S_IFIFO  = 0o010000;
        /// Socket
        const S_IFSOCK = 0o140000;

        // 用户权限
        /// 用户读
        const S_IRUSR  = 0o400;
        /// 用户写
        const S_IWUSR  = 0o200;
        /// 用户执行
        const S_IXUSR  = 0o100;

        // 组权限
        /// 组读
        const S_IRGRP  = 0o040;
        /// 组写
        const S_IWGRP  = 0o020;
        /// 组执行
        const S_IXGRP  = 0o010;

        // 其他用户权限
        /// 其他读
        const S_IROTH  = 0o004;
        /// 其他写
        const S_IWOTH  = 0o002;
        /// 其他执行
        const S_IXOTH  = 0o001;

        // 特殊位
        /// Set UID
        const S_ISUID  = 0o4000;
        /// Set GID
        const S_ISGID  = 0o2000;
        /// Sticky bit
        const S_ISVTX  = 0o1000;
    }
}

impl FileMode {
    /// 检查是否有读权限（暂时只检查用户权限）
    pub fn can_read(&self) -> bool {
        self.contains(FileMode::S_IRUSR)
    }

    /// 检查是否有写权限
    pub fn can_write(&self) -> bool {
        self.contains(FileMode::S_IWUSR)
    }

    /// 检查是否有执行权限
    pub fn can_execute(&self) -> bool {
        self.contains(FileMode::S_IXUSR)
    }
}

/// 轻量级目录项（readdir 返回）
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// 文件名
    pub name: String,
    /// Inode 编号
    pub inode_no: usize,
    /// 文件类型
    pub inode_type: InodeType,
}

/// 文件元数据
#[derive(Debug, Clone)]
pub struct InodeMetadata {
    /// Inode 编号
    pub inode_no: usize,
    /// 文件类型
    pub inode_type: InodeType,
    /// 权限位
    pub mode: FileMode,
    /// 用户 ID
    pub uid: u32,
    /// 组 ID
    pub gid: u32,
    /// 文件大小（字节）
    pub size: usize,
    /// 访问时间
    pub atime: TimeSpec,
    /// 修改时间
    pub mtime: TimeSpec,
    /// 状态改变时间
    pub ctime: TimeSpec,
    /// 硬链接数
    pub nlinks: usize,
    /// 占用的块数（512B 为单位）
    pub blocks: usize,
    /// 设备号（仅对 CharDevice 和 BlockDevice 有效）
    pub rdev: u64,
}

/// 文件系统底层存储接口
pub trait Inode: Send + Sync + Any {
    /// 获取文件元数据
    fn metadata(&self) -> Result<InodeMetadata, FsError>;

    /// 从指定偏移量读取数据
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError>;

    /// 向指定偏移量写入数据
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError>;

    /// 在目录中查找子项
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建文件
    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 在目录中创建子目录
    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError>;

    /// 创建符号链接
    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError>;

    /// 创建硬链接
    fn link(&self, name: &str, target: &Arc<dyn Inode>) -> Result<(), FsError>;

    /// 删除普通文件/链接
    fn unlink(&self, name: &str) -> Result<(), FsError>;

    /// 删除目录
    fn rmdir(&self, name: &str) -> Result<(), FsError>;

    /// 重命名/移动 (原子操作)
    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError>;

    /// 列出目录内容
    fn readdir(&self) -> Result<Vec<DirEntry>, FsError>;

    /// 截断文件到指定大小
    fn truncate(&self, size: usize) -> Result<(), FsError>;

    /// 同步文件数据到存储设备
    fn sync(&self) -> Result<(), FsError>;

    /// 设置 Dentry（可选方法）
    fn set_dentry(&self, _dentry: Weak<Dentry>) {}

    /// 获取 Dentry（可选方法）
    fn get_dentry(&self) -> Option<Arc<Dentry>> {
        None
    }

    /// 是否允许 VFS 缓存该节点的 dentry
    fn cacheable(&self) -> bool {
        true
    }

    /// 向下转型为 &dyn Any，用于支持 downcast
    fn as_any(&self) -> &dyn Any;

    /// 设置文件时间戳
    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError>;

    /// 读取符号链接的目标路径
    fn readlink(&self) -> Result<String, FsError>;

    /// 创建设备文件节点
    fn mknod(&self, name: &str, mode: FileMode, dev: u64) -> Result<Arc<dyn Inode>, FsError>;

    /// 修改文件所有者和组
    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError>;

    /// 修改文件权限模式
    fn chmod(&self, _mode: FileMode) -> Result<(), FsError>;
}

/// 为 `Arc<dyn Inode>` 提供向下转型辅助方法
impl dyn Inode {
    /// 尝试向下转型为具体的 Inode 类型
    pub fn downcast_arc<T: Inode>(self: Arc<Self>) -> Result<Arc<T>, Arc<Self>> {
        if (*self).as_any().is::<T>() {
            // SAFETY: 已经通过 is::<T>() 检查了类型
            unsafe {
                let ptr = Arc::into_raw(self);
                Ok(Arc::from_raw(ptr as *const T))
            }
        } else {
            Err(self)
        }
    }

    /// 尝试获取具体类型的引用
    pub fn downcast_ref<T: Inode>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
