//! 中断管理模块
//!
//! 包含中断控制器驱动实现

pub mod plic;

// Re-export device crate 的 IrqManager 和 IntcDriver
pub use device::irq::{IntcDriver, IrqManager, IRQ_MANAGER};
