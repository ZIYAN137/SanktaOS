//! IPC（进程间通信）模块
//!
//! 本模块聚合了内核中与“任务/进程之间通信”相关的能力实现，主要包含：
//! - 信号：异步事件通知与默认动作处理（实现：`os/src/ipc/signal.rs`）。
//! - 消息队列：以“离散消息”为单位的有界队列通信（实现：`os/src/ipc/message.rs`）。
//! - 共享内存：为多个任务共享同一组物理页，并映射到用户空间（实现：`os/src/ipc/shared_memory.rs`）。
//! - 管道：字节流通信的实现/封装（实现：`crates/vfs/src/impls/pipe_file.rs` 与 `os/src/ipc/pipe.rs`）。
//!
//! # 关于“管道”的位置说明
//!
//! `pipe(2)/pipe2(2)` 系统调用创建并暴露给用户态的管道端点，当前主要由 VFS 层的
//! `crates/vfs/src/impls/pipe_file.rs` 实现（`PipeFile`），系统调用入口在
//! `os/src/kernel/syscall/ipc.rs`。
//!
//! 本目录下的 `os/src/ipc/pipe.rs` 则提供了一个更偏“内核对象”的轻量实现（基于环形缓冲区），
//! 目前与 `pipe2(2)` 并未直接对接；文档与语义应以实际调用路径为准。
#![allow(unused)]
mod message;
mod pipe;
mod shared_memory;
mod signal;

pub use message::*;
pub use pipe::*;
pub use shared_memory::*;
pub use signal::*;
