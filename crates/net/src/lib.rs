//! 内核网络协议栈
//!
//! 此 crate 提供基于 smoltcp 的网络协议栈实现，包括：
//!
//! - Socket 实现（TCP/UDP）
//! - 网络接口管理
//! - 网络配置管理
//! - 与 VFS 层的集成
//!
//! # OS 侧集成
//!
//! `crates/net` 只提供协议栈与 socket 等平台无关能力；OS 侧通常会：
//! - 在启动早期注册 [`NetOps`]（时间与唤醒回调），见 `os/src/net/mod.rs`；
//! - 初始化网卡驱动并创建网络接口（例如 virtio-net），见 `os/src/device/net/virtio_net.rs`；
//! - 在系统调用层将 socket API 映射到 [`SocketFile`] 等实现，见 `os/src/kernel/syscall/network.rs`。

#![no_std]

extern crate alloc;

pub mod config;
pub mod interface;
pub mod ops;
pub mod socket;

// Re-export ops
pub use ops::{NetOps, net_ops, register_net_ops};

// Re-export 主要接口
pub use config::NetworkConfigManager;
pub use interface::{NETWORK_INTERFACE_MANAGER, NetworkInterface};
pub use socket::{
    SocketFile, SocketHandle, create_tcp_socket, create_udp_socket, init_network,
    poll_network_and_dispatch, poll_network_interfaces, register_socket_fd, unregister_socket_fd,
};

// Re-export smoltcp 类型供 syscall 使用
pub use smoltcp::iface::{Context as SmoltcpContext, SocketHandle as SmoltcpSocketHandle};
pub use smoltcp::socket::{tcp, udp};
pub use smoltcp::wire::{
    EthernetAddress, IpAddress, IpCidr, IpEndpoint, IpListenEndpoint, Ipv4Address, Ipv6Address,
};
