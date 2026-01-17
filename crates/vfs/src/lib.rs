//! 内核虚拟文件系统层
//!
//! 此 crate 提供 POSIX 兼容的虚拟文件系统抽象，包括：
//!
//! - [`File`] trait - 文件操作接口
//! - [`Inode`] trait - 索引节点接口
//! - [`FileSystem`] trait - 文件系统接口
//! - [`Dentry`] - 目录项结构
//! - [`FDTable`] - 文件描述符表
//! - 路径解析引擎
//!
//! ## 分层模型：File（会话层）与 Inode（存储层）
//!
//! - **会话层（[`File`]）**：维护“打开文件”的状态（如 offset、open flags），适合实现 `read/write/seek`。
//! - **存储层（[`Inode`]）**：提供底层无状态接口（通常以 `read_at/write_at` 这类随机访问为主），
//!   多个 `File` 可以共享同一个 `Inode`（硬链接/多次 open/dup 等）。
//!
//! ## 路径、挂载与缓存
//!
//! - 路径解析位于 [`path`]，核心入口是 [`vfs_lookup`] 等函数。
//! - 挂载表位于 [`mount`]，支持“同一路径多次挂载”的栈式语义，并在路径解析中自动跟随挂载点。
//! - 目录项缓存（[`DentryCache`]）用于减少重复路径解析开销。
//!
//! ## 运行时依赖
//!
//! VFS 通过 [`ops::VfsOps`] / [`ops::DeviceOps`] 抽象运行时能力（时间、控制台、设备访问、用户态访问保护等），
//! 需要由 `os` crate 实现并在启动早期注册。

#![no_std]
#![allow(clippy::module_inception)]

extern crate alloc;

pub mod dev;
pub mod error;
pub mod ops;

// 先声明基础模块，后续会添加更多
mod adapter;
mod dentry;
mod devno;
mod fd_table;
mod file;
mod file_lock;
mod file_system;
pub mod impls;
mod inode;
mod mount;
mod path;

// Re-export ops
pub use ops::{
    CharDriver, DeviceOps, UserAccessGuard, VfsOps, device_ops, register_device_ops,
    register_vfs_ops, vfs_ops,
};

// Re-export error
pub use error::FsError;

// Re-export dev
pub use dev::{major, makedev, minor};

// Re-export adapter
pub use adapter::{StatExt, StatxExt, inode_type_to_d_type};

// Re-export file
pub use file::File;

// Re-export inode
pub use inode::{DirEntry, FileMode, Inode, InodeMetadata, InodeType};

// Re-export file_system
pub use file_system::{FileSystem, StatFs};

// Re-export dentry
pub use dentry::{DENTRY_CACHE, Dentry, DentryCache};

// Re-export mount
pub use mount::{MOUNT_TABLE, MountFlags, MountPoint, MountTable, get_root_dentry};

// Re-export path
pub use path::{
    PathComponent, normalize_path, parse_path, split_path, vfs_lookup, vfs_lookup_from,
    vfs_lookup_no_follow, vfs_lookup_no_follow_from,
};

// Re-export fd_table
pub use fd_table::{FDTable, FdFlagsExt};

// Re-export file_lock
pub use file_lock::file_lock_manager;

// Re-export devno
pub use devno::{blkdev_major, chrdev_major, get_blkdev_index, get_chrdev_driver, misc_minor};

// Re-export impls
pub use impls::{
    BlkDeviceFile, CharDeviceFile, PipeFile, RegFile, StderrFile, StdinFile, StdoutFile,
    create_stdio_files,
};

// Re-export uapi types for convenience
pub use uapi::fcntl::{FdFlags, OpenFlags, SeekWhence};
pub use uapi::fs::{LinuxDirent64, Stat, Statx};
pub use uapi::time::TimeSpec;
