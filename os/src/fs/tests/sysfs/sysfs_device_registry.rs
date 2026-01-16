//! SysFS 设备注册测试

use super::*;

// 注意：设备注册测试需要实际的设备驱动支持
// 这里主要测试设备注册表的基本功能

#[test_case]
fn test_sysfs_device_registry_basic() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    // 验证设备树结构已创建
    let root = sysfs.root_inode();
    assert!(root.lookup("devices").is_ok());
}

#[test_case]
fn test_sysfs_device_class_structure() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证各类设备类都已创建
    let class_dir = root.lookup("class").unwrap();
    assert!(class_dir.lookup("block").is_ok());
    assert!(class_dir.lookup("net").is_ok());
    assert!(class_dir.lookup("tty").is_ok());
    assert!(class_dir.lookup("input").is_ok());
    assert!(class_dir.lookup("rtc").is_ok());
}

#[test_case]
fn test_sysfs_device_tree_readonly() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let devices_dir = root.lookup("devices").unwrap();

    // 设备目录应该是只读的
    let result = devices_dir.create("test", FileMode::from_bits_truncate(0o644));
    assert!(result.is_err());
}
