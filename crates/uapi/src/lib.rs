//! 与用户空间共用定义和声明
//!
//! 包含常量、类型和函数声明，确保内核和用户空间的一致性

#![no_std]
#![allow(dead_code)]
// uapi 中包含大量与 Linux 兼容的常量/结构体字段定义；逐项补 `///` 噪声较大。
#![allow(missing_docs)]

pub mod cred;
pub mod errno;
pub mod fcntl;
pub mod fs;
pub mod futex;
pub mod ioctl;
pub mod iovec;
pub mod log;
pub mod mm;
pub mod reboot;
pub mod resource;
pub mod sched;
pub mod select;
pub mod signal;
pub mod socket;
pub mod sysinfo;
pub mod time;
pub mod types;
pub mod uts_namespace;
pub mod wait;
