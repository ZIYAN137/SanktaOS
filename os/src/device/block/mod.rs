//! 块设备模块
//!
//! 包含块设备相关的驱动实现

pub mod virtio_blk;

// Re-export device crate 的 BlockDriver trait
pub use device::block::BlockDriver;

// Re-export device crate 的 RamDisk（用于测试与开发）
pub use device::block::RamDisk;
