//! FS 相关操作的 Mock 实现
//!
//! 注意：这里不直接依赖 `fs` crate（避免循环依赖）。
//! `fs` crate 在 `cfg(test)` 下为这些类型实现其 trait（例如 `FsOps`）。

/// Mock 的 FS 运行时操作
pub struct MockFsOps;

impl MockFsOps {
    pub const fn new() -> Self {
        Self
    }
}

/// 全局 Mock 实例
pub static MOCK_FS_OPS: MockFsOps = MockFsOps::new();

