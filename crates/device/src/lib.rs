//! 内核设备驱动框架
//!
//! 此 crate 提供设备驱动的抽象接口和通用实现，包括：
//!
//! - [`Driver`] trait - 设备驱动基础接口
//! - [`BlockDriver`] trait - 块设备驱动接口
//! - [`NetDevice`] trait - 网络设备接口
//! - [`SerialDriver`] trait - 串口驱动接口
//! - [`RtcDriver`] trait - 实时时钟驱动接口
//! - [`Console`] trait - 控制台接口
//! - [`IrqManager`] - 中断管理器
//!
//! # 架构解耦
//!
//! 通过 trait 抽象与架构特定组件解耦：
//! - [`IrqOps`]: 中断启用操作
//!
//! 使用前必须调用 [`register_irq_ops`] 注册实现。

#![no_std]
#![allow(clippy::module_inception)]

extern crate alloc;

pub mod block;
pub mod console;
pub mod driver;
pub mod irq;
pub mod net;
pub mod ops;
pub mod rtc;
pub mod serial;

// Re-export ops
pub use ops::{IrqOps, irq_ops, register_irq_ops};

// Re-export driver
pub use driver::{DeviceType, Driver};

// Re-export irq
pub use irq::{IRQ_MANAGER, IntcDriver, IrqManager};

// Re-export block
pub use block::{BLK_DRIVERS, BlockDriver, RamDisk};

// Re-export net
pub use net::{NETWORK_DEVICES, NetDevice, NetDeviceError, NullNetDevice};

// Re-export serial
pub use serial::{SERIAL_DRIVERS, SerialDriver};

// Re-export rtc
pub use rtc::{DateTime, RTC_DRIVERS, RtcDriver};

// Re-export console
pub use console::{CONSOLES, Console, MAIN_CONSOLE};

// Re-export 全局驱动列表
pub use driver::{CMDLINE, DRIVERS, register_driver};
