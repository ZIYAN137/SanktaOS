# IPC 概述

IPC（进程间通信）用于在任务/进程之间传递数据、事件与共享状态。当前实现主要覆盖：
- 管道（Pipe）：字节流通信（主要以“文件端点”的形式通过 VFS 暴露）。
- 消息（Message）：离散消息的有界队列通信。
- 共享内存（Shared Memory）：共享物理页并映射到用户空间。
- 信号（Signal）：异步事件通知、默认动作与用户态 handler 调度。

## 源码导览（以源码 rustdoc 为准）
- IPC 聚合入口：`os/src/ipc/mod.rs`
- 管道系统调用入口：`os/src/kernel/syscall/ipc.rs`（`pipe2`）
- 管道文件实现：`crates/vfs/src/impls/pipe_file.rs`（`PipeFile`）
- 消息队列：`os/src/ipc/message.rs`
- 共享内存：`os/src/ipc/shared_memory.rs`
- 信号：`os/src/ipc/signal.rs`

## 说明
- 本目录只保留概览类文档；实现细节与语义以对应源码文件的 rustdoc 注释为准。
