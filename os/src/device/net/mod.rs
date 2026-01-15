//! 网络设备驱动模块
//!
//! 管理和初始化各种网络设备

pub mod net_device;
pub mod virtio_net;

// Re-export device crate 的网络设备类型
pub use device::net::{
    add_network_device, format_mac_address, get_net_devices, NetDevice, NetDeviceError,
    NullNetDevice, NETWORK_DEVICES,
};

// Re-export VirtioNetDevice
pub use net_device::VirtioNetDevice;
