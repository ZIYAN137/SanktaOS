//! 字符设备文件的 File trait 实现

use alloc::sync::Arc;
use sync::SpinLock;
use uapi::ioctl::Termios;

use crate::dev::{major, minor};
use crate::devno::{chrdev_major, get_chrdev_driver, misc_minor};
use crate::{
    CharDriver, Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence, UserAccessGuard,
};

/// 字符设备文件
pub struct CharDeviceFile {
    /// 关联的 dentry
    pub dentry: Arc<Dentry>,

    /// 关联的 inode
    pub inode: Arc<dyn Inode>,

    /// 设备号
    dev: u64,

    /// 设备驱动（缓存）
    driver: Option<Arc<dyn CharDriver>>,

    /// 打开标志位
    pub flags: OpenFlags,

    /// 偏移量（某些字符设备可能需要）
    offset: SpinLock<usize>,

    /// 终端属性（用于 TTY 设备）
    termios: SpinLock<Termios>,

    /// 终端窗口大小（用于 TTY 设备）
    winsize: SpinLock<uapi::ioctl::WinSize>,
}

impl CharDeviceFile {
    const ICRNL: u32 = 0x0100;
    const INLCR: u32 = 0x0040;
    const IGNCR: u32 = 0x0080;
    const OPOST: u32 = 0x0001;
    const ONLCR: u32 = 0x0004;
    const ICANON: u32 = 0x0002;
    const ECHO: u32 = 0x0008;

    #[inline]
    fn map_input_byte(mut ch: u8, iflag: u32) -> Option<u8> {
        if (iflag & Self::IGNCR) != 0 && ch == b'\r' {
            return None;
        }
        if (iflag & Self::ICRNL) != 0 && ch == b'\r' {
            ch = b'\n';
        } else if (iflag & Self::INLCR) != 0 && ch == b'\n' {
            ch = b'\r';
        }
        Some(ch)
    }

    #[inline]
    fn echo_byte(&self, ch: u8) {
        if let Some(ref driver) = self.driver {
            driver.write(&[ch]);
        }
    }

    /// 创建新的字符设备文件
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Result<Self, FsError> {
        let inode = dentry.inode.clone();
        let metadata = inode.metadata()?;
        let dev = metadata.rdev;

        let driver = get_chrdev_driver(dev);

        let maj = major(dev);
        if driver.is_none() && maj != chrdev_major::MEM {
            return Err(FsError::NoDevice);
        }

        Ok(Self {
            dentry,
            inode,
            dev,
            driver,
            flags,
            offset: SpinLock::new(0),
            termios: SpinLock::new(Termios::default()),
            winsize: SpinLock::new(uapi::ioctl::WinSize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
        })
    }

    /// 处理内存设备的读操作
    fn mem_device_read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let min = minor(self.dev);
        match min {
            3 => Ok(0), // /dev/null
            5 => {
                // /dev/zero
                buf.fill(0);
                Ok(buf.len())
            }
            8 | 9 => {
                // /dev/random, /dev/urandom
                // 简单实现：使用固定种子
                let mut seed = 12345u32;
                for byte in buf.iter_mut() {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    *byte = (seed >> 16) as u8;
                }
                Ok(buf.len())
            }
            _ => Err(FsError::NoDevice),
        }
    }

    /// 处理内存设备的写操作
    fn mem_device_write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let min = minor(self.dev);
        match min {
            3 | 5 => Ok(buf.len()), // /dev/null, /dev/zero
            _ => Err(FsError::NoDevice),
        }
    }
}

impl File for CharDeviceFile {
    fn readable(&self) -> bool {
        self.flags.readable()
    }

    fn writable(&self) -> bool {
        self.flags.writable()
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }

        let maj = major(self.dev);

        if maj == chrdev_major::MEM {
            return self.mem_device_read(buf);
        }

        if let Some(ref driver) = self.driver {
            let term = *self.termios.lock();
            let canonical = (term.c_lflag & Self::ICANON) != 0;
            let do_echo = (term.c_lflag & Self::ECHO) != 0;
            let is_nonblock = self.flags.contains(OpenFlags::O_NONBLOCK);

            let mut count = 0usize;

            if is_nonblock {
                if let Some(b) = driver.try_read() {
                    if let Some(mapped) = Self::map_input_byte(b, term.c_iflag) {
                        if do_echo {
                            self.echo_byte(mapped);
                        }
                        buf[count] = mapped;
                        count += 1;
                    }
                    while count < buf.len() {
                        if let Some(nb) = driver.try_read() {
                            if let Some(mapped) = Self::map_input_byte(nb, term.c_iflag) {
                                if do_echo {
                                    self.echo_byte(mapped);
                                }
                                buf[count] = mapped;
                                count += 1;
                                if canonical && mapped == b'\n' {
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    Ok(count)
                } else {
                    Err(FsError::WouldBlock)
                }
            } else {
                loop {
                    let b = match driver.try_read() {
                        Some(bb) => bb,
                        None => {
                            core::hint::spin_loop();
                            continue;
                        }
                    };
                    if let Some(mapped) = Self::map_input_byte(b, term.c_iflag) {
                        if do_echo {
                            self.echo_byte(mapped);
                        }
                        buf[count] = mapped;
                        count += 1;
                        if !canonical || mapped == b'\n' || count >= buf.len() {
                            break;
                        }
                    }
                }
                Ok(count)
            }
        } else {
            Err(FsError::NoDevice)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }

        let maj = major(self.dev);

        if maj == chrdev_major::MEM {
            return self.mem_device_write(buf);
        }

        if let Some(ref driver) = self.driver {
            let term = *self.termios.lock();
            let post = (term.c_oflag & Self::OPOST) != 0;
            let onlcr = (term.c_oflag & Self::ONLCR) != 0;
            if post && onlcr {
                for &ch in buf {
                    if ch == b'\n' {
                        driver.write(&[b'\r', b'\n']);
                    } else {
                        driver.write(&[ch]);
                    }
                }
            } else {
                driver.write(buf);
            }
            Ok(buf.len())
        } else {
            Err(FsError::NoDevice)
        }
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    fn lseek(&self, _offset: isize, _whence: SeekWhence) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn offset(&self) -> usize {
        *self.offset.lock()
    }

    fn flags(&self) -> OpenFlags {
        self.flags.clone()
    }

    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.inode.clone())
    }

    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry.clone())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        let maj = major(self.dev);

        match maj {
            chrdev_major::CONSOLE | chrdev_major::TTY => self.console_ioctl(request, arg),
            chrdev_major::MISC => self.misc_ioctl(request, arg),
            _ => Err(FsError::NotSupported),
        }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl CharDeviceFile {
    /// 控制台设备 ioctl 处理
    fn console_ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
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

                    core::ptr::write_bytes(
                        termios_ptr as *mut u8,
                        0,
                        core::mem::size_of::<Termios>(),
                    );

                    let termios = *self.termios.lock();
                    core::ptr::write_volatile(termios_ptr, termios);
                }
                Ok(0)
            }

            TCSETS | TCSETSW | TCSETSF => {
                if arg == 0 {
                    return Ok(-EINVAL as isize);
                }

                {
                    let _guard = UserAccessGuard::new();
                    let termios_ptr = arg as *const Termios;
                    if termios_ptr.is_null() {
                        return Ok(-EINVAL as isize);
                    }

                    unsafe {
                        let new_termios = core::ptr::read_volatile(termios_ptr);
                        *self.termios.lock() = new_termios;
                    }
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

                    let winsize = *self.winsize.lock();
                    core::ptr::write_volatile(winsize_ptr, winsize);
                }
                Ok(0)
            }

            TIOCSWINSZ => {
                if arg == 0 {
                    return Ok(-EINVAL as isize);
                }

                {
                    let _guard = UserAccessGuard::new();
                    let winsize_ptr = arg as *const uapi::ioctl::WinSize;
                    if winsize_ptr.is_null() {
                        return Ok(-EINVAL as isize);
                    }

                    unsafe {
                        let new_winsize = core::ptr::read_volatile(winsize_ptr);
                        *self.winsize.lock() = new_winsize;
                    }
                }
                Ok(0)
            }

            _ => Ok(-ENOTTY as isize),
        }
    }

    /// MISC 设备 ioctl 处理
    fn misc_ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        use uapi::errno::EINVAL;
        use uapi::ioctl::*;

        let min = minor(self.dev);

        if min == misc_minor::RTC {
            match request {
                RTC_RD_TIME => {
                    if arg == 0 {
                        return Ok(-EINVAL as isize);
                    }

                    if let Some(ref driver) = self.driver {
                        // RTC ioctl 需要通过 DeviceOps 实现
                        // 这里简化处理，返回不支持
                        return Err(FsError::NotSupported);
                    }
                    Err(FsError::NoDevice)
                }
                _ => Err(FsError::NotSupported),
            }
        } else {
            Err(FsError::NotSupported)
        }
    }
}
