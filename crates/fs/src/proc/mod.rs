//! ProcFS - 进程信息伪文件系统
//!
//! 该模块提供了一个与 **Linux /proc 兼容的虚拟文件系统**，用于导出内核和进程信息。

pub mod generators;
pub mod inode;
pub mod proc;

pub use inode::{ContentGenerator, ProcInode, ProcInodeContent};
pub use proc::ProcFS;
