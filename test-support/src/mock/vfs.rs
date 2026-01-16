//! VFS 相关操作的 Mock 实现
//!
//! 注意：这里不直接依赖 `vfs` crate（避免循环依赖）。
//! `vfs` crate 在 `cfg(test)` 下为这些类型实现其 trait（例如 `VfsOps` / `DeviceOps`）。

/// Mock 的 VFS 操作
///
/// 提供最小可用语义以支持单元测试（默认返回空/None，并将副作用变为 no-op）。
pub struct MockVfsOps;

impl MockVfsOps {
    pub const fn new() -> Self {
        Self
    }
}

/// Mock 的设备操作
///
/// 读写块设备相关接口默认返回失败/no-op。
pub struct MockDeviceOps;

impl MockDeviceOps {
    pub const fn new() -> Self {
        Self
    }
}

/// 全局 Mock 实例
pub static MOCK_VFS_OPS: MockVfsOps = MockVfsOps::new();
pub static MOCK_DEVICE_OPS: MockDeviceOps = MockDeviceOps::new();

