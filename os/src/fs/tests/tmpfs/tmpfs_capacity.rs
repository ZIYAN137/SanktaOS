//! Tmpfs 容量限制测试

use super::*;
use alloc::vec;

#[test_case]
fn test_tmpfs_capacity_unlimited() {
    // 创建无限制容量的 tmpfs
    let fs = create_test_tmpfs_unlimited();
    let root = fs.root_inode();

    // 写入大文件（模拟）
    let file = root
        .create("large.bin", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xAA; 1024 * 1024]; // 1 MB
    let written = file.write_at(0, &data).unwrap();
    assert!(written == data.len());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    assert!(metadata.size == 1024 * 1024);
}

#[test_case]
fn test_tmpfs_capacity_limited() {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入接近容量限制的数据
    let file = root
        .create("test.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xBB; 512 * 1024]; // 512 KB
    let result = file.write_at(0, &data);
    assert!(result.is_ok());
}

#[test_case]
fn test_tmpfs_capacity_exceed() {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 尝试写入超过容量的数据
    let file = root
        .create("huge.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xCC; 2 * 1024 * 1024]; // 2 MB (超过限制)
    let result = file.write_at(0, &data);

    // 应该失败或部分写入
    assert!(result.is_err() || result.unwrap() < data.len());
}

// TODO: tmpfs 容量限制可能未严格实施，需要验证实现

#[test_case]
fn test_tmpfs_capacity_after_delete() {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入数据
    let file = root
        .create("temp.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xEE; 512 * 1024]; // 512 KB
    assert!(file.write_at(0, &data).is_ok());

    // 删除文件
    assert!(root.unlink("temp.dat").is_ok());

    // 应该能够再次写入相同大小的文件
    let file2 = root
        .create("new.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let result = file2.write_at(0, &data);
    assert!(result.is_ok());
}

#[test_case]
fn test_tmpfs_capacity_truncate() {
    // 创建 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入数据
    let file = root
        .create("test.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xFF; 512 * 1024]; // 512 KB
    assert!(file.write_at(0, &data).is_ok());

    // 截断文件
    assert!(file.truncate(1024).is_ok());

    // 验证新大小
    let metadata = file.metadata().unwrap();
    assert!(metadata.size == 1024);

    // 应该能够写入更多数据（因为空间被释放）
    let file2 = root
        .create("new.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let result = file2.write_at(0, &data);
    assert!(result.is_ok());
}
