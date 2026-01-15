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

pub mod ops;
pub mod driver;
pub mod irq;
pub mod block;
pub mod net;
pub mod serial;
pub mod rtc;
pub mod console;

// Re-export ops
pub use ops::{register_irq_ops, irq_ops, IrqOps};

// Re-export driver
pub use driver::{DeviceType, Driver};

// Re-export irq
pub use irq::{IrqManager, IntcDriver, IRQ_MANAGER};

// Re-export block
pub use block::{BlockDriver, RamDisk, BLK_DRIVERS};

// Re-export net
pub use net::{NetDevice, NetDeviceError, NullNetDevice, NETWORK_DEVICES};

// Re-export serial
pub use serial::{SerialDriver, SERIAL_DRIVERS};

// Re-export rtc
pub use rtc::{RtcDriver, DateTime, RTC_DRIVERS};

// Re-export console
pub use console::{Console, CONSOLES, MAIN_CONSOLE};

// Re-export 全局驱动列表
pub use driver::{DRIVERS, register_driver, CMDLINE};
