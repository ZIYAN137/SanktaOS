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
