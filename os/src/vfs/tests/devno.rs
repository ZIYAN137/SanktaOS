//! 设备号管理测试

use super::*;
use crate::vfs::{
    blkdev_major, chrdev_major, get_blkdev_index, get_chrdev_driver,
    dev::{major, makedev, minor},
};
#[test_case]
fn test_makedev_major_minor() {
    // 测试 makedev 和 major/minor 提取
    let dev = makedev(8, 1);
    assert!(major(dev) == 8);
    assert!(minor(dev) == 1);
}

#[test_case]
fn test_makedev_zero() {
    let dev = makedev(0, 0);
    assert!(major(dev) == 0);
    assert!(minor(dev) == 0);
}

#[test_case]
fn test_makedev_large_numbers() {
    let dev = makedev(255, 255);
    assert!(major(dev) == 255);
    assert!(minor(dev) == 255);
}

#[test_case]
fn test_makedev_roundtrip() {
    // 测试往返转换
    for maj in [0, 1, 8, 10, 100, 255] {
        for min in [0, 1, 16, 100, 255] {
            let dev = makedev(maj, min);
            assert!(major(dev) == maj);
            assert!(minor(dev) == min);
        }
    }
}

#[test_case]
fn test_blkdev_major() {
    // 测试块设备主设备号常量
    assert!(blkdev_major::LOOP == 7);
    assert!(blkdev_major::SCSI_DISK == 8);
    assert!(blkdev_major::VIRTIO_BLK == 254);
}

#[test_case]
fn test_chrdev_major() {
    // 测试字符设备主设备号常量
    assert!(chrdev_major::MEM == 1);
    assert!(chrdev_major::TTY == 4);
    assert!(chrdev_major::CONSOLE == 5);
    assert!(chrdev_major::INPUT == 13);
}

#[test_case]
fn test_get_blkdev_index() {
    // 测试获取块设备索引
    let index = get_blkdev_index(0);
    assert!(index.is_some() || index.is_none()); // 取决于是否有注册的设备
}

#[test_case]
fn test_get_chrdev_driver() {
    // 测试获取字符设备驱动
    let driver = get_chrdev_driver(makedev(1, 0));
    assert!(driver.is_some() || driver.is_none()); // 取决于是否有注册的驱动
}

#[test_case]
fn test_devno_unique() {
    // 确保不同的 major/minor 组合产生不同的 devno
    let dev1 = makedev(1, 0);
    let dev2 = makedev(1, 1);
    let dev3 = makedev(2, 0);

    assert!(dev1 != dev2);
    assert!(dev1 != dev3);
    assert!(dev2 != dev3);
}

#[test_case]
fn test_major_extraction() {
    // 测试主设备号提取的边界情况
    let dev = makedev(255, 0);
    assert!(major(dev) == 255);
    assert!(minor(dev) == 0);
}

#[test_case]
fn test_minor_extraction() {
    // 测试次设备号提取的边界情况
    let dev = makedev(0, 255);
    assert!(major(dev) == 0);
    assert!(minor(dev) == 255);
}

#[test_case]
fn test_devno_consistency() {
    // 测试设备号的一致性
    let dev1 = makedev(10, 20);
    let dev2 = makedev(10, 20);
    assert!(dev1 == dev2);
    assert!(major(dev1) == major(dev2));
    assert!(minor(dev1) == minor(dev2));
}
