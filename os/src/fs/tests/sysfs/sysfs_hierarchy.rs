//! SysFS 目录层次结构测试

use super::*;

#[test_case]
fn test_sysfs_class_directory() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/class 应该存在
    let class_dir = root.lookup("class");
    assert!(class_dir.is_ok());

    let inode = class_dir.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Directory);
}

#[test_case]
fn test_sysfs_devices_directory() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/devices 应该存在
    let devices_dir = root.lookup("devices");
    assert!(devices_dir.is_ok());

    let inode = devices_dir.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Directory);
}

#[test_case]
fn test_sysfs_kernel_directory() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/kernel 应该存在
    let kernel_dir = root.lookup("kernel");
    assert!(kernel_dir.is_ok());

    let inode = kernel_dir.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Directory);
}

#[test_case]
fn test_sysfs_class_block() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/class/block 应该存在
    let class_dir = root.lookup("class").unwrap();
    let block_dir = class_dir.lookup("block");
    assert!(block_dir.is_ok());
}

#[test_case]
fn test_sysfs_class_net() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let net_dir = class_dir.lookup("net");
    assert!(net_dir.is_ok());
}

#[test_case]
fn test_sysfs_class_tty() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let tty_dir = class_dir.lookup("tty");
    assert!(tty_dir.is_ok());
}

#[test_case]
fn test_sysfs_class_input() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let input_dir = class_dir.lookup("input");
    assert!(input_dir.is_ok());
}

#[test_case]
fn test_sysfs_class_rtc() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let rtc_dir = class_dir.lookup("rtc");
    assert!(rtc_dir.is_ok());
}

#[test_case]
fn test_sysfs_root_readdir() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let entries = root.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();

    // 应该包含基本目录
    assert!(names.contains(&"class"));
    assert!(names.contains(&"devices"));
    assert!(names.contains(&"kernel"));
}

#[test_case]
fn test_sysfs_class_readdir() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();
    let class_dir = root.lookup("class").unwrap();

    let entries = class_dir.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();

    // 应该包含各类设备类
    assert!(names.contains(&"block"));
    assert!(names.contains(&"net"));
    assert!(names.contains(&"tty"));
    assert!(names.contains(&"input"));
    assert!(names.contains(&"rtc"));
}

#[test_case]
fn test_sysfs_hierarchy_depth() {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 测试多层目录访问
    let class_dir = root.lookup("class").unwrap();
    let block_dir = class_dir.lookup("block").unwrap();

    let metadata = block_dir.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Directory);
}
