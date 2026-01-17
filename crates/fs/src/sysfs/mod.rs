//! Sysfs 虚拟文件系统
//!
//! 提供与 Linux 兼容的 sysfs 接口,用于暴露设备和内核信息。
//!
//! ## 目录结构
//!
//! 典型结构包括：
//!
//! - `/sys/class/*`：按类别组织（block/net/tty/input/rtc 等）
//! - `/sys/devices/*`：设备树（platform/devices 等）
//!
//! ## 构建器与设备注册表
//!
//! - `device_registry`：从设备层收集设备信息（块设备、网络设备、TTY、RTC 等）
//! - `builders`：将设备信息“装配”为 sysfs 目录树与符号链接
//!
//! 当前实现以只读导出为主，写入/热插拔等能力可按需扩展。

mod builders;
mod device_registry;
mod inode;
mod sysfs;

pub use device_registry::{find_block_device, find_net_device};
pub use sysfs::SysFS;
