//! 内存模拟块设备

use super::BlockDriver;
use crate::driver::{DeviceType, Driver};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use sync::SpinLock;

/// 内存模拟的块设备
///
/// 用于测试和开发
pub struct RamDisk {
    /// 存储数据
    data: SpinLock<Vec<u8>>,

    /// 块大小
    block_size: usize,

    /// 设备 ID
    device_id: usize,
}

impl RamDisk {
    /// 创建指定大小的内存磁盘
    pub fn new(size: usize, block_size: usize, device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            data: SpinLock::new(vec![0u8; size]),
            block_size,
            device_id,
        })
    }

    /// 从字节数组创建
    pub fn from_bytes(data: Vec<u8>, block_size: usize, device_id: usize) -> Arc<Self> {
        Arc::new(Self {
            data: SpinLock::new(data),
            block_size,
            device_id,
        })
    }

    /// 获取原始数据（用于调试）
    pub fn raw_data(&self) -> Vec<u8> {
        self.data.lock().clone()
    }

    /// 获取设备 ID
    pub fn device_id(&self) -> usize {
        self.device_id
    }
}

impl Driver for RamDisk {
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        false // RamDisk 不处理中断
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn get_id(&self) -> String {
        alloc::format!("ramdisk_{}", self.device_id)
    }

    fn as_block(&self) -> Option<&dyn BlockDriver> {
        Some(self)
    }
}

// 实现 BlockDriver trait
impl BlockDriver for RamDisk {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool {
        if buf.len() != self.block_size {
            return false;
        }

        let data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return false;
        }

        buf.copy_from_slice(&data[offset..offset + self.block_size]);
        true
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> bool {
        if buf.len() != self.block_size {
            return false;
        }

        let mut data = self.data.lock();
        let offset = block_id * self.block_size;

        if offset + self.block_size > data.len() {
            return false;
        }

        data[offset..offset + self.block_size].copy_from_slice(buf);
        true
    }

    fn flush(&self) -> bool {
        true // 内存设备无需 flush
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn total_blocks(&self) -> usize {
        self.data.lock().len() / self.block_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use sync::ArchOps;

    struct DummyArchOps;

    impl ArchOps for DummyArchOps {
        unsafe fn read_and_disable_interrupts(&self) -> usize {
            0
        }

        unsafe fn restore_interrupts(&self, _flags: usize) {}

        fn sstatus_sie(&self) -> usize {
            0
        }

        fn cpu_id(&self) -> usize {
            0
        }

        fn max_cpu_count(&self) -> usize {
            1
        }
    }

    static DUMMY_ARCH_OPS: DummyArchOps = DummyArchOps;
    // 0 = uninit, 1 = initializing, 2 = ready
    static SYNC_INIT: AtomicUsize = AtomicUsize::new(0);

    fn init_sync_arch_ops() {
        match SYNC_INIT.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                // Safety: tests use a single global dummy ArchOps.
                unsafe { sync::register_arch_ops(&DUMMY_ARCH_OPS) };
                SYNC_INIT.store(2, Ordering::Release);
            }
            Err(_) => {
                while SYNC_INIT.load(Ordering::Acquire) != 2 {
                    core::hint::spin_loop();
                }
            }
        }
    }

    #[test]
    fn test_ramdisk_read_write_roundtrip() {
        init_sync_arch_ops();
        let rd = RamDisk::new(4096, 512, 1);
        assert_eq!(rd.block_size(), 512);
        assert_eq!(rd.total_blocks(), 8);

        let mut wbuf = [0u8; 512];
        wbuf[0] = 0xAA;
        wbuf[511] = 0x55;
        assert!(rd.write_block(3, &wbuf));

        let mut rbuf = [0u8; 512];
        assert!(rd.read_block(3, &mut rbuf));
        assert_eq!(rbuf, wbuf);

        // Other blocks remain zero.
        let mut rbuf2 = [0u8; 512];
        assert!(rd.read_block(2, &mut rbuf2));
        assert_eq!(rbuf2, [0u8; 512]);
    }

    #[test]
    fn test_ramdisk_bounds_and_wrong_buf_size() {
        init_sync_arch_ops();
        let rd = RamDisk::new(1024, 512, 1);
        assert_eq!(rd.total_blocks(), 2);

        let mut bad_read = [0u8; 16];
        assert!(!rd.read_block(0, &mut bad_read));

        let bad_write = [0u8; 16];
        assert!(!rd.write_block(0, &bad_write));

        let mut ok_read = [0u8; 512];
        assert!(!rd.read_block(2, &mut ok_read)); // out of range

        let ok_write = [0u8; 512];
        assert!(!rd.write_block(2, &ok_write)); // out of range
    }
}
