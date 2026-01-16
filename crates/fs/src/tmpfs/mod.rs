//! Tmpfs - 内存临时文件系统
//!
//! 该模块提供了一个**完全驻留在内存中的文件系统**，支持完整的 POSIX 语义。

mod inode;
mod tmpfs;

pub use inode::TmpfsInode;
pub use tmpfs::TmpFs;
