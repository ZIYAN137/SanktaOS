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

#![no_std]
#![doc = "文件系统实现"]

extern crate alloc;

pub mod ext4;
pub mod ops;
pub mod proc;
pub mod sysfs;
pub mod tmpfs;

pub use ext4::{BlockDeviceAdapter, Ext4FileSystem, Ext4Inode};
pub use ops::{fs_ops, register_fs_ops, FsOps, MemoryAreaInfo, MountInfo, TaskInfo, TaskState, VmStats};
pub use proc::{ContentGenerator, ProcFS, ProcInode, ProcInodeContent};
pub use sysfs::{find_block_device, find_net_device, SysFS};
pub use tmpfs::{TmpFs, TmpfsInode};
