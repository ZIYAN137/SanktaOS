//! 设备抽象层（OS 侧入口）
//!
//! SanktaOS 的设备子系统分为两部分：
//! 1) `crates/device`：提供与平台无关的驱动抽象（traits）以及全局注册表/中断管理器等通用设施。
//! 2) `os/src/device`：在 OS 侧重导出 `crates/device`，并实现“平台相关”的设备初始化流程，
//!    例如：设备树探测、VirtIO 传输层、具体设备驱动、中断控制器驱动等。
//!
//! # 初始化流程（概览）
//!
//! 典型启动路径（以 `os/src/arch/*/platform/*` 为入口）大致为：
//! - `init_device_ops()`：注册 `crates/device` 需要的架构回调（如启用中断）；
//! - `*_driver_init()`：注册设备树 `compatible` → 探测函数（例如 PLIC、virtio-mmio 等）；
//! - `device_tree::init()`：遍历设备树并按顺序初始化设备（通常先中断控制器，再其他设备）；
//! - 驱动初始化过程中会把自身登记到全局表（如 `DRIVERS`、`BLK_DRIVERS`、`NETWORK_DEVICES` 等），
//!   并在需要时通过 `IRQ_MANAGER` 注册中断派发。
//!
//! 说明：具体顺序与平台相关，建议以对应平台文件的调用链为准（例如 `os/src/arch/riscv/platform/virt.rs`）。

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
