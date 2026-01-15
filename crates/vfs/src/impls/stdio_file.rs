//! 标准 I/O 文件实现
//!
//! 提供标准输入、输出、错误输出的文件接口，直接操作控制台，不依赖 Inode。

use alloc::sync::Arc;
use sync::SpinLock;
use uapi::ioctl::Termios;

use crate::{vfs_ops, File, FileMode, FsError, InodeMetadata, InodeType, UserAccessGuard};

/// 全局终端设置（所有标准I/O文件共享）
static STDIO_TERMIOS: SpinLock<Termios> = SpinLock::new(Termios::DEFAULT);

/// 全局窗口大小（所有标准I/O文件共享）
static STDIO_WINSIZE: SpinLock<uapi::ioctl::WinSize> = SpinLock::new(uapi::ioctl::WinSize {
    ws_row: 24,
    ws_col: 80,
    ws_xpixel: 0,
    ws_ypixel: 0,
});

/// 标准输入文件
///
/// 从控制台读取输入，行缓冲模式。
pub struct StdinFile;

impl File for StdinFile {
    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        const ICRNL: u32 = 0x0100;
        const INLCR: u32 = 0x0040;
        const IGNCR: u32 = 0x0080;
        const ICANON: u32 = 0x0002;
        const ECHO: u32 = 0x0008;

        let term = *STDIO_TERMIOS.lock();
        let canonical = (term.c_lflag & ICANON) != 0;
        let do_echo = (term.c_lflag & ECHO) != 0;

        let mut count = 0usize;

        while count < buf.len() {
            let ch_opt = vfs_ops().console_getchar();
            let mut ch = match ch_opt {
                Some(c) => c,
                None => break,
            };

            if (term.c_iflag & IGNCR) != 0 && ch == b'\r' {
                continue;
            }
            if (term.c_iflag & ICRNL) != 0 && ch == b'\r' {
                ch = b'\n';
            } else if (term.c_iflag & INLCR) != 0 && ch == b'\n' {
                ch = b'\r';
            }

            if do_echo {
                vfs_ops().console_putchar(ch);
            }

            buf[count] = ch;
            count += 1;

            if !canonical || ch == b'\n' {
                break;
            }
        }

        Ok(count)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IRUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: vfs_ops().timespec_now(),
            mtime: vfs_ops().timespec_now(),
            ctime: vfs_ops().timespec_now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// 标准输出文件
///
/// 输出到控制台，全缓冲模式。
pub struct StdoutFile;

impl File for StdoutFile {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if let Ok(s) = core::str::from_utf8(buf) {
            vfs_ops().console_write_str(s);
        } else {
            for &byte in buf {
                vfs_ops().console_putchar(byte);
            }
        }
        Ok(buf.len())
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 1,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: vfs_ops().timespec_now(),
            mtime: vfs_ops().timespec_now(),
            ctime: vfs_ops().timespec_now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// 标准错误输出文件
///
/// 输出到控制台（与 stdout 相同），无缓冲模式。
pub struct StderrFile;

impl File for StderrFile {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if let Ok(s) = core::str::from_utf8(buf) {
            vfs_ops().console_write_str(s);
        } else {
            for &byte in buf {
                vfs_ops().console_putchar(byte);
            }
        }
        Ok(buf.len())
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 2,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: vfs_ops().timespec_now(),
            mtime: vfs_ops().timespec_now(),
            ctime: vfs_ops().timespec_now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// 通用的 stdio ioctl 实现
fn stdio_ioctl(request: u32, arg: usize) -> Result<isize, FsError> {
    use uapi::errno::{EINVAL, ENOTTY};
    use uapi::ioctl::*;

    match request {
        TCGETS => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = UserAccessGuard::new();
                let termios_ptr = arg as *mut Termios;
                if termios_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                core::ptr::write_bytes(termios_ptr as *mut u8, 0, core::mem::size_of::<Termios>());

                let termios = *STDIO_TERMIOS.lock();
                core::ptr::write_volatile(termios_ptr, termios);
            }
            Ok(0)
        }

        TCSETS | TCSETSW | TCSETSF => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = UserAccessGuard::new();
                let termios_ptr = arg as *const Termios;
                if termios_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                let new_termios = core::ptr::read_volatile(termios_ptr);
                *STDIO_TERMIOS.lock() = new_termios;
            }
            Ok(0)
        }

        TIOCGWINSZ => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = UserAccessGuard::new();
                let winsize_ptr = arg as *mut uapi::ioctl::WinSize;
                if winsize_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                core::ptr::write_bytes(
                    winsize_ptr as *mut u8,
                    0,
                    core::mem::size_of::<uapi::ioctl::WinSize>(),
                );

                let winsize = *STDIO_WINSIZE.lock();
                core::ptr::write_volatile(winsize_ptr, winsize);
            }
            Ok(0)
        }

        TIOCSWINSZ => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = UserAccessGuard::new();
                let winsize_ptr = arg as *const uapi::ioctl::WinSize;
                if winsize_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                let new_winsize = core::ptr::read_volatile(winsize_ptr);
                *STDIO_WINSIZE.lock() = new_winsize;
            }
            Ok(0)
        }

        _ => Ok(-ENOTTY as isize),
    }
}

/// 创建标准 I/O 文件对象
///
/// 返回: 三元组 (stdin, stdout, stderr)
pub fn create_stdio_files() -> (Arc<dyn File>, Arc<dyn File>, Arc<dyn File>) {
    (
        Arc::new(StdinFile),
        Arc::new(StdoutFile),
        Arc::new(StderrFile),
    )
}
