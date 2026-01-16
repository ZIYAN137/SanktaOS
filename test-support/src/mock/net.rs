//! 网络相关操作的 Mock 实现
//!
//! 注意：这里不直接依赖 `net` crate（避免循环依赖）。
//! `net` crate 在 `cfg(test)` 下为这些类型实现其 trait（例如 `NetOps`）。

/// Mock 的网络运行时操作
pub struct MockNetOps;

impl MockNetOps {
    pub const fn new() -> Self {
        Self
    }
}

/// 全局 Mock 实例
pub static MOCK_NET_OPS: MockNetOps = MockNetOps::new();

