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

pub mod ops;
pub mod error;
pub mod dev;

// 先声明基础模块，后续会添加更多
mod adapter;
mod file;
mod inode;
mod file_system;
mod dentry;
mod mount;
mod path;
mod fd_table;
mod file_lock;
mod devno;
pub mod impls;

// Re-export ops
pub use ops::{
    register_vfs_ops, vfs_ops, VfsOps,
    register_device_ops, device_ops, DeviceOps, CharDriver,
    UserAccessGuard,
};

// Re-export error
pub use error::FsError;

// Re-export dev
pub use dev::{major, minor, makedev};

// Re-export adapter
pub use adapter::{inode_type_to_d_type, StatExt, StatxExt};

// Re-export file
pub use file::File;

// Re-export inode
pub use inode::{DirEntry, FileMode, Inode, InodeMetadata, InodeType};

// Re-export file_system
pub use file_system::{FileSystem, StatFs};

// Re-export dentry
pub use dentry::{Dentry, DentryCache, DENTRY_CACHE};

// Re-export mount
pub use mount::{get_root_dentry, MountFlags, MountPoint, MountTable, MOUNT_TABLE};

// Re-export path
pub use path::{
    normalize_path, parse_path, split_path, vfs_lookup, vfs_lookup_from,
    vfs_lookup_no_follow, vfs_lookup_no_follow_from, PathComponent,
};

// Re-export fd_table
pub use fd_table::{FDTable, FdFlagsExt};

// Re-export file_lock
pub use file_lock::file_lock_manager;

// Re-export devno
pub use devno::{blkdev_major, chrdev_major, get_blkdev_index, get_chrdev_driver, misc_minor};

// Re-export impls
pub use impls::{
    create_stdio_files, BlkDeviceFile, CharDeviceFile, PipeFile, RegFile, StderrFile, StdinFile,
    StdoutFile,
};

// Re-export uapi types for convenience
pub use uapi::fcntl::{FdFlags, OpenFlags, SeekWhence};
pub use uapi::fs::{LinuxDirent64, Stat, Statx};
pub use uapi::time::TimeSpec;
