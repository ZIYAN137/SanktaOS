//! 设备相关操作的 Mock 实现
//!
//! 注意：这里不直接依赖 `device` crate（避免循环依赖）。
//! `device` crate 在 `cfg(test)` 下为这些类型实现其 trait（例如 `IrqOps`）。

/// Mock 的中断操作
pub struct MockIrqOps;

impl MockIrqOps {
    pub const fn new() -> Self {
        Self
    }
}

/// 全局 Mock 实例
pub static MOCK_IRQ_OPS: MockIrqOps = MockIrqOps::new();

