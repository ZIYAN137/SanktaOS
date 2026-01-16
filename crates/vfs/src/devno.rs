//! 设备号到驱动的硬编码映射
//!
//! 冷插拔系统的简化实现：所有设备号到驱动的映射都通过硬编码规则完成。

use alloc::sync::Arc;

use crate::dev::{major, minor};
use crate::{CharDriver, device_ops};

/// 标准字符设备 major 号
pub mod chrdev_major {
    /// /dev/null, /dev/zero 等
    pub const MEM: u32 = 1;
    /// /dev/tty*, /dev/ttyS*
    pub const TTY: u32 = 4;
    /// /dev/console
    pub const CONSOLE: u32 = 5;
    /// /dev/misc/* (rtc=135)
    pub const MISC: u32 = 10;
    /// /dev/input/*
    pub const INPUT: u32 = 13;
}

/// MISC 设备 minor 号
pub mod misc_minor {
    /// RTC 设备
    pub const RTC: u32 = 135;
}

/// 标准块设备 major 号
pub mod blkdev_major {
    /// /dev/loop*
    pub const LOOP: u32 = 7;
    /// /dev/sd*
    pub const SCSI_DISK: u32 = 8;
    /// /dev/vd*
    pub const VIRTIO_BLK: u32 = 254;
}

/// 查找字符设备驱动（硬编码规则）
pub fn get_chrdev_driver(dev: u64) -> Option<Arc<dyn CharDriver>> {
    device_ops().get_chrdev_driver(dev)
}

/// 查找块设备驱动索引（硬编码规则）
pub fn get_blkdev_index(dev: u64) -> Option<usize> {
    let maj = major(dev);
    let min = minor(dev);

    match maj {
        blkdev_major::VIRTIO_BLK => {
            // VirtIO 块设备：minor 直接对应 BLK_DRIVERS 索引
            Some(min as usize)
        }
        blkdev_major::SCSI_DISK => {
            // SCSI 磁盘：每个磁盘占用 16 个 minor
            let disk_idx = (min / 16) as usize;
            Some(disk_idx)
        }
        blkdev_major::LOOP => {
            // 回环设备：暂不支持
            None
        }
        _ => None,
    }
}
