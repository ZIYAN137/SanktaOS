//! SysFS 设备构建器测试

use super::*;

// 设备构建器测试
// 这些测试验证设备树构建逻辑

#[test_case]
fn test_sysfs_builders_block_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证块设备类存在
    let class_dir = root.lookup("class").unwrap();
    let block_dir = class_dir.lookup("block");
    assert!(block_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_net_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let net_dir = class_dir.lookup("net");
    assert!(net_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_tty_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let tty_dir = class_dir.lookup("tty");
    assert!(tty_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_input_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let input_dir = class_dir.lookup("input");
    assert!(input_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_rtc_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let rtc_dir = class_dir.lookup("rtc");
    assert!(rtc_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_kernel_info() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证内核信息目录存在
    let kernel_dir = root.lookup("kernel");
    assert!(kernel_dir.is_ok());
}

#[test_case]
fn test_sysfs_builders_platform_devices() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证设备目录存在
    let devices_dir = root.lookup("devices");
    assert!(devices_dir.is_ok());
}
