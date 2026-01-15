//! VFS 操作 trait 实现
//!
//! 此模块为 vfs crate 的 VfsOps 和 DeviceOps trait 提供 os crate 的具体实现。

use alloc::sync::Arc;
use uapi::time::TimeSpec;
use vfs::{chrdev_major, CharDriver, Dentry, DeviceOps, VfsOps};

use crate::config::DEFAULT_MAX_FDS;
use crate::device::serial::SerialDriver;
use crate::device::{BLK_DRIVERS, SERIAL_DRIVERS};
use crate::time_ext::timespec_now;

/// VFS 操作实现
struct VfsOpsImpl;

impl VfsOps for VfsOpsImpl {
    fn current_cwd(&self) -> Option<Arc<Dentry>> {
        crate::kernel::current_task()
            .lock()
            .fs
            .lock()
            .cwd
            .clone()
    }

    fn current_root(&self) -> Option<Arc<Dentry>> {
        crate::kernel::current_task()
            .lock()
            .fs
            .lock()
            .root
            .clone()
    }

    fn default_max_fds(&self) -> usize {
        DEFAULT_MAX_FDS
    }

    fn timespec_now(&self) -> TimeSpec {
        timespec_now()
    }

    fn enter_user_access(&self) {
        #[cfg(target_arch = "riscv64")]
        unsafe {
            riscv::register::sstatus::set_sum();
        }
        #[cfg(target_arch = "loongarch64")]
        {
            // LoongArch 不需要特殊处理
        }
    }

    fn exit_user_access(&self) {
        #[cfg(target_arch = "riscv64")]
        unsafe {
            riscv::register::sstatus::clear_sum();
        }
        #[cfg(target_arch = "loongarch64")]
        {
            // LoongArch 不需要特殊处理
        }
    }

    fn console_getchar(&self) -> Option<u8> {
        crate::console::getchar()
    }

    fn console_putchar(&self, c: u8) {
        crate::console::putchar(c);
    }

    fn console_write_str(&self, s: &str) {
        crate::console::write_str(s);
    }
}

/// 设备操作实现
struct DeviceOpsImpl;

impl DeviceOps for DeviceOpsImpl {
    fn get_chrdev_driver(&self, dev: u64) -> Option<Arc<dyn CharDriver>> {
        use vfs::dev::{major, minor};

        let maj = major(dev);
        let min = minor(dev);

        match maj {
            chrdev_major::TTY => {
                // TTY 设备：ttyS0 的 minor 是 64，映射到 SERIAL_DRIVERS[0]
                // 标准 Linux 中 ttyS* 的 minor 从 64 开始
                let idx = if min >= 64 { min - 64 } else { min };
                let drivers = SERIAL_DRIVERS.read();
                let driver = drivers.get(idx as usize)?.clone();
                Some(Arc::new(SerialDriverWrapper(driver)))
            }
            chrdev_major::CONSOLE => {
                // console 设备：直接使用 SERIAL_DRIVERS[0]
                let drivers = SERIAL_DRIVERS.read();
                let driver = drivers.first()?.clone();
                Some(Arc::new(SerialDriverWrapper(driver)))
            }
            _ => None,
        }
    }

    fn get_blkdev_index(&self, dev: u64) -> Option<usize> {
        vfs::get_blkdev_index(dev)
    }

    fn read_block(&self, idx: usize, block_id: usize, buf: &mut [u8]) -> bool {
        let drivers = BLK_DRIVERS.read();
        if let Some(driver) = drivers.get(idx) {
            driver.read_block(block_id, buf)
        } else {
            false
        }
    }

    fn write_block(&self, idx: usize, block_id: usize, buf: &[u8]) -> bool {
        let drivers = BLK_DRIVERS.read();
        if let Some(driver) = drivers.get(idx) {
            driver.write_block(block_id, buf)
        } else {
            false
        }
    }

    fn blkdev_total_blocks(&self, idx: usize) -> usize {
        let drivers = BLK_DRIVERS.read();
        if let Some(driver) = drivers.get(idx) {
            driver.total_blocks()
        } else {
            0
        }
    }
}

/// SerialDriver 到 CharDriver 的适配器
struct SerialDriverWrapper(Arc<dyn SerialDriver>);

impl CharDriver for SerialDriverWrapper {
    fn try_read(&self) -> Option<u8> {
        self.0.try_read()
    }

    fn write(&self, data: &[u8]) {
        self.0.write(data);
    }

    fn ioctl(&self, _request: u32, _arg: usize) -> Result<isize, i32> {
        Err(uapi::errno::ENOTTY)
    }
}

/// 全局 VFS 操作实例
static VFS_OPS: VfsOpsImpl = VfsOpsImpl;

/// 全局设备操作实例
static DEVICE_OPS: DeviceOpsImpl = DeviceOpsImpl;

/// 初始化 VFS 操作
///
/// 必须在使用 VFS 之前调用此函数
pub fn init_vfs_ops() {
    // SAFETY: 在单线程启动阶段调用
    unsafe {
        vfs::register_vfs_ops(&VFS_OPS);
        vfs::register_device_ops(&DEVICE_OPS);
    }
}
