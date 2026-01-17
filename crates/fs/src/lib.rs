//! # 文件系统模块 (FS)
//!
//! 本模块提供了多种具体的文件系统实现，通过实现VFS的`FileSystem`和`Inode` trait与虚拟文件系统层集成。
//!
//! ## 支持的文件系统
//!
//! - **[tmpfs](tmpfs)**: 临时文件系统(纯内存)
//! - **[procfs](proc)**: 进程信息伪文件系统
//! - **[sysfs](sysfs)**: 系统设备伪文件系统
//! - **[ext4]**: Linux Ext4文件系统
//!
//! ## 与运行时解耦（FsOps）
//!
//! FS 层通过 [`ops::FsOps`] 抽象从 `os` 运行时获取的能力（时间、页大小、任务信息、挂载信息等），
//! 由 `os` crate 实现并在启动早期通过 [`ops::register_fs_ops`] 注册。
//!
//! - `tmpfs` 需要页大小与当前时间
//! - `procfs` 需要任务/内存/挂载信息以生成 `/proc/*`
//! - `sysfs` 需要设备注册表信息以生成 `/sys/*`

#![no_std]
#![doc = "文件系统实现"]

extern crate alloc;

pub mod ext4;
pub mod ops;
pub mod proc;
pub mod sysfs;
pub mod tmpfs;

pub use ext4::{BlockDeviceAdapter, Ext4FileSystem, Ext4Inode};
pub use ops::{
    FsOps, MemoryAreaInfo, MountInfo, TaskInfo, TaskState, VmStats, fs_ops, register_fs_ops,
};
pub use proc::{ContentGenerator, ProcFS, ProcInode, ProcInodeContent};
pub use sysfs::{SysFS, find_block_device, find_net_device};
pub use tmpfs::{TmpFs, TmpfsInode};
