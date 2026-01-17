# 网络概述

网络子系统基于 `smoltcp` 提供基础协议栈能力，并以“socket 文件”的形式通过 VFS/系统调用暴露给用户态。

本目录只保留概览类文档；实现细节与 API 语义以对应源码文件的 rustdoc 注释为准。

## 源码导览（以 rustdoc 为准）

- OS 侧包装与初始化：`os/src/net/mod.rs`（注册 `NetOps`）
- 网络系统调用：`os/src/kernel/syscall/network.rs`
- 设备驱动（virtio-net）：`os/src/device/net/virtio_net.rs`
- 协议栈与 socket 实现：`crates/net/src/`
  - 接口管理：`crates/net/src/interface.rs`
  - 配置管理：`crates/net/src/config.rs`
  - socket/VFS 集成：`crates/net/src/socket.rs`
  - OS 侧回调抽象：`crates/net/src/ops.rs`

## 运行机制（概览）

- 驱动初始化后会创建 `NetworkInterface` 并加入接口管理器；
- 系统调用侧创建/操作 `SocketFile`，并注册到协议栈的 socket 集合；
- 通过轮询（poll）驱动协议栈推进收发，并在需要时唤醒 `poll/select` 等等待者。
