//! 中断管理与中断控制器驱动（OS 侧）
//!
//! `crates/device` 提供通用的 `IrqManager` / `IntcDriver` 抽象以及全局 `IRQ_MANAGER`；
//! OS 侧在此目录下提供具体的中断控制器驱动实现（例如 RISC-V 的 PLIC）。

pub mod plic;

// Re-export device crate 的 IrqManager 和 IntcDriver
pub use device::irq::{IRQ_MANAGER, IntcDriver, IrqManager};
