//! 串行设备驱动模块
//!
//! 包含各种串行设备驱动程序的实现模块。

pub mod keyboard;
pub mod uart16550;
pub mod virtio_console;

// Re-export device crate 的 SerialDriver trait
pub use device::serial::{SerialDriver, SERIAL_DRIVERS};
