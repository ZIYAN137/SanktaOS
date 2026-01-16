//! 内核网络协议栈
//!
//! 此 crate 提供基于 smoltcp 的网络协议栈实现，包括：
//!
//! - Socket 实现（TCP/UDP）
//! - 网络接口管理
//! - 网络配置管理
//! - 与 VFS 层的集成

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
