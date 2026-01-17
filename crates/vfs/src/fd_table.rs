//! 文件描述符表
//!
//! 该模块实现了进程级的文件描述符管理，提供 POSIX 兼容的文件描述符操作。
//!
//! 约定与语义（与用户态常见预期保持一致）：
//!
//! - `alloc()` 通常分配“最小可用 fd”（0/1/2 在用户进程中多用于 stdio）
//! - `dup/dup2` 等操作会共享底层 `Arc<dyn File>`（因此可能共享 offset）
//! - `FD_CLOEXEC` 用于控制 exec 时是否关闭 fd（由 `FdFlags` 表示）

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use sync::SpinLock;
use uapi::fcntl::{FdFlags, OpenFlags};

use crate::{File, FsError, vfs_ops};

/// 文件描述符表
pub struct FDTable {
    /// 文件描述符数组
    files: SpinLock<Vec<Option<Arc<dyn File>>>>,
    /// 文件描述符标志数组
    fd_flags: SpinLock<Vec<FdFlags>>,
    /// 最大文件描述符数量
    max_fds: usize,
}

/// FdFlags 扩展 trait
pub trait FdFlagsExt {
    /// 从 OpenFlags 中提取 FD 标志
    fn from_open_flags(flags: OpenFlags) -> Self;
}

impl FdFlagsExt for FdFlags {
    fn from_open_flags(flags: OpenFlags) -> Self {
        let mut fd_flags = FdFlags::empty();
        if flags.contains(OpenFlags::O_CLOEXEC) {
            fd_flags |= FdFlags::CLOEXEC;
        }
        fd_flags
    }
}

impl fmt::Debug for FDTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let files = self.files.lock();
        let used = files.iter().filter(|slot| slot.is_some()).count();
        f.debug_struct("FDTable")
            .field("max_fds", &self.max_fds)
            .field("slots", &files.len())
            .field("used", &used)
            .finish()
    }
}

impl FDTable {
    /// 创建新的文件描述符表
    pub fn new() -> Self {
        Self {
            files: SpinLock::new(Vec::new()),
            fd_flags: SpinLock::new(Vec::new()),
            max_fds: vfs_ops().default_max_fds(),
        }
    }

    /// 取走并清空所有已打开的文件描述符
    pub fn take_all(&self) -> Vec<(usize, Arc<dyn File>)> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        let mut out = Vec::new();
        for (fd, slot) in files.iter_mut().enumerate() {
            if let Some(file) = slot.take() {
                out.push((fd, file));
            }
        }
        for f in fd_flags.iter_mut() {
            *f = FdFlags::empty();
        }
        out
    }

    /// 分配一个新的文件描述符（默认无 FD 标志）
    pub fn alloc(&self, file: Arc<dyn File>) -> Result<usize, FsError> {
        self.alloc_with_flags(file, FdFlags::empty())
    }

    /// 分配一个新的文件描述符并指定 FD 标志
    pub fn alloc_with_flags(&self, file: Arc<dyn File>, flags: FdFlags) -> Result<usize, FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        // 查找最小可用 FD
        for (fd, slot) in files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                fd_flags[fd] = flags;
                return Ok(fd);
            }
        }

        // 如果没有空闲槽位，扩展数组
        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }

        files.push(Some(file));
        fd_flags.push(flags);
        Ok(fd)
    }

    /// 在指定的 FD 位置安装文件（默认无 FD 标志）
    pub fn install_at(&self, fd: usize, file: Arc<dyn File>) -> Result<(), FsError> {
        self.install_at_with_flags(fd, file, FdFlags::empty())
    }

    /// 在指定的 FD 位置安装文件并指定 FD 标志
    pub fn install_at_with_flags(
        &self,
        fd: usize,
        file: Arc<dyn File>,
        flags: FdFlags,
    ) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= self.max_fds {
            return Err(FsError::InvalidArgument);
        }

        // 扩展数组到指定大小
        while files.len() <= fd {
            files.push(None);
            fd_flags.push(FdFlags::empty());
        }

        files[fd] = Some(file);
        fd_flags[fd] = flags;
        Ok(())
    }

    /// 获取文件对象
    pub fn get(&self, fd: usize) -> Result<Arc<dyn File>, FsError> {
        let files = self.files.lock();
        files
            .get(fd)
            .and_then(|f| f.clone())
            .ok_or(FsError::BadFileDescriptor)
    }

    /// 关闭文件描述符
    pub fn close(&self, fd: usize) -> Result<(), FsError> {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        files[fd] = None;
        fd_flags[fd] = FdFlags::empty();
        Ok(())
    }

    /// 复制文件描述符
    pub fn dup(&self, old_fd: usize) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    /// 复制文件描述符，新 fd >= min_fd（F_DUPFD 语义）
    pub fn dup_from(&self, old_fd: usize, min_fd: usize, flags: FdFlags) -> Result<usize, FsError> {
        let file = self.get(old_fd)?;
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        while files.len() < min_fd {
            files.push(None);
            fd_flags.push(FdFlags::empty());
        }

        for (fd, slot) in files.iter_mut().enumerate().skip(min_fd) {
            if slot.is_none() {
                *slot = Some(file);
                fd_flags[fd] = flags;
                return Ok(fd);
            }
        }

        let fd = files.len();
        if fd >= self.max_fds {
            return Err(FsError::TooManyOpenFiles);
        }

        files.push(Some(file));
        fd_flags.push(flags);
        Ok(fd)
    }

    /// 复制文件描述符到指定位置
    pub fn dup2(&self, old_fd: usize, new_fd: usize) -> Result<usize, FsError> {
        if old_fd == new_fd {
            self.get(old_fd)?;
            return Ok(new_fd);
        }
        self.dup3(old_fd, new_fd, OpenFlags::empty())
    }

    /// 复制文件描述符到指定位置（dup3 语义）
    pub fn dup3(&self, old_fd: usize, new_fd: usize, flags: OpenFlags) -> Result<usize, FsError> {
        if old_fd == new_fd {
            return Err(FsError::InvalidArgument);
        }

        let file = self.get(old_fd)?;
        let _ = self.close(new_fd);
        let fd_flags = FdFlags::from_open_flags(flags);
        self.install_at_with_flags(new_fd, file, fd_flags)?;
        Ok(new_fd)
    }

    /// 克隆整个文件描述符表（用于 fork）
    pub fn clone_table(&self) -> Self {
        let files = self.files.lock().clone();
        let fd_flags = self.fd_flags.lock().clone();
        Self {
            files: SpinLock::new(files),
            fd_flags: SpinLock::new(fd_flags),
            max_fds: self.max_fds,
        }
    }

    /// 关闭所有带有 CLOEXEC 标志的文件（用于 exec）
    pub fn close_exec(&self) {
        let mut files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        for (slot, flags) in files.iter_mut().zip(fd_flags.iter_mut()) {
            if flags.contains(FdFlags::CLOEXEC) {
                *slot = None;
                *flags = FdFlags::empty();
            }
        }
    }

    /// 获取文件描述符标志 (F_GETFD)
    pub fn get_fd_flags(&self, fd: usize) -> Result<FdFlags, FsError> {
        let files = self.files.lock();
        let fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        Ok(fd_flags[fd])
    }

    /// 设置文件描述符标志 (F_SETFD)
    pub fn set_fd_flags(&self, fd: usize, flags: FdFlags) -> Result<(), FsError> {
        let files = self.files.lock();
        let mut fd_flags = self.fd_flags.lock();

        if fd >= files.len() || files[fd].is_none() {
            return Err(FsError::BadFileDescriptor);
        }

        fd_flags[fd] = flags;
        Ok(())
    }
}

impl Default for FDTable {
    fn default() -> Self {
        Self::new()
    }
}
