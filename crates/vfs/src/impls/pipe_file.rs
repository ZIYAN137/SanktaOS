//! 管道文件实现
//!
//! 管道是流式单向通信设备，读端和写端分别由两个 [`PipeFile`] 实例表示。

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use sync::SpinLock;

use crate::{File, FileMode, FsError, InodeMetadata, InodeType, OpenFlags, TimeSpec};

/// 管道环形缓冲区
///
/// 容量默认 4KB（POSIX 最小 512 字节）。
struct PipeRingBuffer {
    /// 内部缓冲区
    buffer: VecDeque<u8>,
    /// 缓冲区容量
    capacity: usize,
    /// 写端引用计数 (用于检测写端关闭)
    write_end_count: usize,
    /// 读端引用计数 (用于检测读端关闭)
    read_end_count: usize,
}

impl PipeRingBuffer {
    const DEFAULT_CAPACITY: usize = 4096;
    const MIN_CAPACITY: usize = 4096;
    const MAX_CAPACITY: usize = 1048576;

    fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(Self::DEFAULT_CAPACITY),
            capacity: Self::DEFAULT_CAPACITY,
            write_end_count: 0,
            read_end_count: 0,
        }
    }

    fn get_capacity(&self) -> usize {
        self.capacity
    }

    fn set_capacity(&mut self, new_capacity: usize) -> Result<(), FsError> {
        if new_capacity < Self::MIN_CAPACITY || new_capacity > Self::MAX_CAPACITY {
            return Err(FsError::InvalidArgument);
        }

        if new_capacity < self.buffer.len() {
            return Err(FsError::InvalidArgument);
        }

        self.capacity = new_capacity;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.buffer.is_empty() {
            if self.write_end_count == 0 {
                return Ok(0);
            }
            return Ok(0);
        }

        let nread = buf.len().min(self.buffer.len());
        for i in 0..nread {
            buf[i] = self.buffer.pop_front().unwrap();
        }

        Ok(nread)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        if self.read_end_count == 0 {
            return Err(FsError::BrokenPipe);
        }

        let available = self.capacity - self.buffer.len();
        if available == 0 {
            return Ok(0);
        }

        let nwrite = buf.len().min(available);
        for &byte in &buf[..nwrite] {
            self.buffer.push_back(byte);
        }

        Ok(nwrite)
    }
}

/// 管道文件实现
///
/// 特点:
/// - 单向数据流 (读端和写端分别创建两个 PipeFile 实例)
/// - 流式设备 (无 offset 概念,不支持 lseek)
/// - 不依赖 Inode (纯内存结构)
pub struct PipeFile {
    /// 共享的环形缓冲区
    buffer: Arc<SpinLock<PipeRingBuffer>>,
    /// 文件端点类型
    end_type: PipeEnd,
    /// 打开标志位 (支持 O_NONBLOCK 等)
    flags: SpinLock<OpenFlags>,
    /// 异步 I/O 所有者 PID
    owner: SpinLock<Option<i32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipeEnd {
    Read,
    Write,
}

impl PipeFile {
    /// 创建管道对 (返回 [读端, 写端])
    pub fn create_pair() -> (Self, Self) {
        let buffer = Arc::new(SpinLock::new(PipeRingBuffer::new()));

        {
            let mut buf = buffer.lock();
            buf.read_end_count = 1;
            buf.write_end_count = 1;
        }

        let read_end = Self {
            buffer: buffer.clone(),
            end_type: PipeEnd::Read,
            flags: SpinLock::new(OpenFlags::empty()),
            owner: SpinLock::new(None),
        };

        let write_end = Self {
            buffer,
            end_type: PipeEnd::Write,
            flags: SpinLock::new(OpenFlags::empty()),
            owner: SpinLock::new(None),
        };

        (read_end, write_end)
    }

    /// 设置文件状态标志 (F_SETFL)
    pub fn set_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        let mut flags = self.flags.lock();
        *flags = new_flags;
        Ok(())
    }

    /// 获取管道大小 (F_GETPIPE_SZ)
    pub fn get_pipe_size(&self) -> usize {
        self.buffer.lock().get_capacity()
    }

    /// 设置管道大小 (F_SETPIPE_SZ)
    pub fn set_pipe_size(&self, new_size: usize) -> Result<(), FsError> {
        self.buffer.lock().set_capacity(new_size)
    }
}

impl File for PipeFile {
    fn readable(&self) -> bool {
        if self.end_type != PipeEnd::Read {
            return false;
        }
        let buf = self.buffer.lock();
        !buf.buffer.is_empty() || buf.write_end_count == 0
    }

    fn writable(&self) -> bool {
        if self.end_type != PipeEnd::Write {
            return false;
        }
        let buf = self.buffer.lock();
        buf.read_end_count > 0 && buf.buffer.len() < buf.capacity
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.end_type != PipeEnd::Read {
            return Err(FsError::InvalidArgument);
        }

        let mut ring_buf = self.buffer.lock();
        ring_buf.read(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if self.end_type != PipeEnd::Write {
            return Err(FsError::InvalidArgument);
        }

        let mut ring_buf = self.buffer.lock();
        ring_buf.write(buf)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::Fifo,
            size: 0,
            mode: FileMode::S_IFIFO | FileMode::S_IRUSR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            atime: TimeSpec::zero(),
            mtime: TimeSpec::zero(),
            ctime: TimeSpec::zero(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn flags(&self) -> OpenFlags {
        *self.flags.lock()
    }

    fn set_status_flags(&self, new_flags: OpenFlags) -> Result<(), FsError> {
        self.set_flags(new_flags)
    }

    fn get_pipe_size(&self) -> Result<usize, FsError> {
        Ok(self.buffer.lock().get_capacity())
    }

    fn set_pipe_size(&self, size: usize) -> Result<(), FsError> {
        self.buffer.lock().set_capacity(size)
    }

    fn get_owner(&self) -> Result<i32, FsError> {
        Ok(self.owner.lock().unwrap_or(0))
    }

    fn set_owner(&self, pid: i32) -> Result<(), FsError> {
        *self.owner.lock() = if pid == 0 { None } else { Some(pid) };
        Ok(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl Drop for PipeFile {
    fn drop(&mut self) {
        let mut buf = self.buffer.lock();
        match self.end_type {
            PipeEnd::Read => buf.read_end_count -= 1,
            PipeEnd::Write => buf.write_end_count -= 1,
        }
    }
}
