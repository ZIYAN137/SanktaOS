//! ProcFS 动态文件测试

use super::*;

#[test_case]
fn test_procfs_meminfo_exists() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 查找 meminfo 文件
    let meminfo = root.lookup("meminfo");
    assert!(meminfo.is_ok());

    let inode = meminfo.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::File);
    assert!(metadata.mode.bits() & 0o777 == 0o444); // r--r--r--
}

#[test_case]
fn test_procfs_meminfo_read() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let meminfo = root.lookup("meminfo").unwrap();

    // 读取内容
    let mut buf = [0u8; 1024];
    let read = meminfo.read_at(0, &mut buf);
    assert!(read.is_ok());

    let bytes_read = read.unwrap();
    assert!(bytes_read > 0);

    // 内容应该包含一些内存相关的信息
    let content = core::str::from_utf8(&buf[..bytes_read]);
    assert!(content.is_ok());
}

#[test_case]
fn test_procfs_uptime_exists() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let uptime = root.lookup("uptime");
    assert!(uptime.is_ok());

    let inode = uptime.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::File);
}

#[test_case]
fn test_procfs_uptime_read() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let uptime = root.lookup("uptime").unwrap();

    let mut buf = [0u8; 256];
    let read = uptime.read_at(0, &mut buf);
    assert!(read.is_ok());

    let bytes_read = read.unwrap();
    assert!(bytes_read > 0);

    let content = core::str::from_utf8(&buf[..bytes_read]);
    assert!(content.is_ok());
}

#[test_case]
fn test_procfs_cpuinfo_exists() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let cpuinfo = root.lookup("cpuinfo");
    assert!(cpuinfo.is_ok());
}

#[test_case]
fn test_procfs_cpuinfo_read() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let cpuinfo = root.lookup("cpuinfo").unwrap();

    let mut buf = [0u8; 1024];
    let read = cpuinfo.read_at(0, &mut buf);
    assert!(read.is_ok());

    let bytes_read = read.unwrap();
    assert!(bytes_read > 0);
}

#[test_case]
fn test_procfs_mounts_exists() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let mounts = root.lookup("mounts");
    assert!(mounts.is_ok());
}

#[test_case]
fn test_procfs_mounts_read() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let mounts = root.lookup("mounts").unwrap();

    let mut buf = [0u8; 2048];
    let read = mounts.read_at(0, &mut buf);
    assert!(read.is_ok());
}

#[test_case]
fn test_procfs_dynamic_file_size_zero() {
    // proc 文件的大小总是报告为 0
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let meminfo = root.lookup("meminfo").unwrap();

    let metadata = meminfo.metadata().unwrap();
    assert!(metadata.size == 0);
}

#[test_case]
fn test_procfs_dynamic_file_readonly() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let meminfo = root.lookup("meminfo").unwrap();

    // 动态文件应该是只读的
    let result = meminfo.write_at(0, b"test");
    assert!(result.is_err());
}

#[test_case]
fn test_procfs_dynamic_file_regenerate() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let uptime = root.lookup("uptime").unwrap();

    // 读取两次，内容可能不同（因为是动态生成的）
    let mut buf1 = [0u8; 256];
    let read1 = uptime.read_at(0, &mut buf1).unwrap();

    let mut buf2 = [0u8; 256];
    let read2 = uptime.read_at(0, &mut buf2).unwrap();

    // 至少应该成功读取
    assert!(read1 > 0);
    assert!(read2 > 0);
}
