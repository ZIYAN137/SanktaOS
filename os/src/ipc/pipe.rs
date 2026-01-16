//! 管道模块（内核对象版）
//!
//! 该文件实现了一个基于环形缓冲区的“内核内管道对象”：
//! - 一对端点（读端/写端）共享同一个 `PipeRingBuffer`；
//! - 读写通过 `UserBuffer` 与用户空间交互；
//! - 当前实现不包含等待队列/阻塞语义：读空时直接返回 0，写满时停止写入并返回已写入字节数。
//!
//! # 重要说明：这不是 `pipe2(2)` 的主要实现
//!
//! 用户态通过 `pipe(2)/pipe2(2)` 获得的 fd 端点，当前由 VFS 的 `PipeFile`
//! （见 `crates/vfs/src/impls/pipe_file.rs`）实现，并由 `os/src/kernel/syscall/ipc.rs`
//! 暴露为系统调用。两者语义/行为可能不同，阅读与调试时请以实际调用路径为准。

use alloc::{
    sync::{Arc, Weak},
    vec::{self, Vec},
};

use crate::{
    sync::Mutex,
    util::{ring_buffer::RingBuffer, user_buffer::UserBuffer},
};

/// 创建一个管道，返回读端和写端
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(Mutex::new(PipeRingBuffer::new()));
    let read_end = Arc::new(Pipe::init_read_end(buffer.clone()));
    let write_end = Arc::new(Pipe::init_write_end(buffer.clone()));
    buffer.lock().set_write_end(&write_end);
    (read_end, write_end)
}

/// 管道结构体
pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
}

impl Pipe {
    /// 创建一个读端
    pub fn init_read_end(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }

    /// 创建一个写端
    pub fn init_write_end(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }

    /// 从管道中读取数据到用户缓冲区
    /// 返回值: 实际读取的字节数
    pub fn read(&self, buf: UserBuffer) -> usize {
        if buf.is_empty() {
            return 0;
        }
        let cap = buf.len();
        let mut data = Vec::with_capacity(cap);
        {
            let mut rb = self.buffer.lock();
            while data.len() < cap {
                match rb.read_byte() {
                    Some(b) => data.push(b),
                    None => break,
                }
            }
        }
        let n = data.len();
        // Safety:
        // - sys_read 调用处构造的 UserBuffer 已验证其指针在用户空间且长度为 n；
        // - data 是内核侧 Vec，与用户缓冲不重叠；
        // - 拷贝内部会临时开启 SUM。
        unsafe { buf.copy_to_user(&data) };
        n
    }

    /// 向管道中写入数据
    /// 返回值: 实际写入的字节数
    pub fn write(&self, buf: UserBuffer) -> usize {
        if buf.is_empty() {
            return 0;
        }
        // Safety:
        // - sys_write 调用处构造的 UserBuffer 已验证其指针在用户空间且长度 buf.len()；
        // - 读取源是用户空间只读区域；内部会临时开启 SUM。
        let data = unsafe { buf.copy_from_user() };
        let mut wrote = 0;
        {
            let mut rb = self.buffer.lock();
            for byte in data {
                if rb.write_byte(byte).is_ok() {
                    wrote += 1;
                } else {
                    break;
                }
            }
        }
        wrote
    }
}

/// 管道环形缓冲区
pub(crate) struct PipeRingBuffer {
    buffer: RingBuffer,
    write_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    /// 创建一个新的 PipeRingBuffer
    pub fn new() -> Self {
        Self {
            buffer: RingBuffer::new(),
            write_end: None,
        }
    }

    /// 读取一个字节
    pub fn read_byte(&mut self) -> Option<u8> {
        self.buffer.read_byte()
    }

    /// 写入一个字节
    pub fn write_byte(&mut self, byte: u8) -> Result<(), ()> {
        self.buffer.write_byte(byte)
    }

    /// 可用空间
    pub fn available_space(&self) -> usize {
        self.buffer.available_space()
    }

    /// 设置写端
    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }

    /// 检查是否所有写端都已被丢弃
    pub fn all_write_ends_dropped(&self) -> bool {
        self.write_end
            .as_ref()
            .map_or(true, |weak| weak.upgrade().is_none())
    }
}
