//! 设备抽象层
//!
//! 此模块重新导出 device crate 的所有公共接口，并提供 os crate 特定的实现。

// Re-export device crate
pub use ::device::*;

// os-specific 模块
mod ops_impl;

#[macro_use]
pub mod bus;
pub mod console;
pub mod device_tree;
pub mod gpu;
pub mod input;
pub mod irq;
pub mod net;
pub mod rtc;
pub mod serial;
pub mod virtio_hal;

pub mod block;

// Re-export ops_impl 初始化函数
pub use ops_impl::init_device_ops;

// Re-export console 初始化函数
pub use console::init as init_console;
