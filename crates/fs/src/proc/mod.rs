//! ProcFS - 进程信息伪文件系统
//!
//! 该模块提供了一个与 **Linux /proc 兼容的虚拟文件系统**，用于导出内核和进程信息。
//!
//! ## 生成器机制（ContentGenerator）
//!
//! procfs 中的很多文件内容由生成器在读取时动态生成：
//!
//! - 系统级：`/proc/meminfo`、`/proc/cpuinfo`、`/proc/uptime`、`/proc/mounts` 等
//! - 进程级：`/proc/[pid]/stat`、`/proc/[pid]/status`、`/proc/[pid]/maps`、`/proc/[pid]/cmdline` 等
//!
//! 生成器通常通过 [`crate::ops::fs_ops`] 获取任务/内存/挂载信息，并序列化为 Linux 风格文本。

/// procfs 文件内容生成器集合。
pub mod generators;
pub mod inode;
pub mod proc;

pub use inode::{ContentGenerator, ProcInode, ProcInodeContent};
pub use proc::ProcFS;
