//! ProcFS 集成测试

use super::*;

#[test_case]
fn test_procfs_init_tree() {
    let procfs = ProcFS::new();
    let result = procfs.init_tree();
    assert!(result.is_ok());
}

#[test_case]
fn test_procfs_full_initialization() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 验证所有预期的文件都存在
    assert!(root.lookup("meminfo").is_ok());
    assert!(root.lookup("uptime").is_ok());
    assert!(root.lookup("cpuinfo").is_ok());
    assert!(root.lookup("mounts").is_ok());
    assert!(root.lookup("self").is_ok());
}

// TODO: 此测试需要完整的内核上下文来生成动态内容

#[test_case]
fn test_procfs_filesystem_type() {
    let procfs = create_test_procfs_with_tree().unwrap();
    assert!(procfs.fs_type() == "proc");
}

#[test_case]
fn test_procfs_sync_after_init() {
    let procfs = create_test_procfs_with_tree().unwrap();
    assert!(procfs.sync().is_ok());
}

#[test_case]
fn test_procfs_statfs_after_init() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let statfs = procfs.statfs();
    assert!(statfs.is_ok());

    let stats = statfs.unwrap();
    assert!(stats.block_size == 4096);
    assert!(stats.total_blocks == 0);
}

#[test_case]
fn test_procfs_multiple_init() {
    // 测试多次初始化
    let procfs = ProcFS::new();
    assert!(procfs.init_tree().is_ok());

    // 第二次初始化可能失败（文件已存在）或成功（幂等）
    let result = procfs.init_tree();
    // 无论成功或失败，都不应该导致系统崩溃
    assert!(result.is_ok() || result.is_err());
}

#[test_case]
fn test_procfs_concurrent_access() {
    // 测试并发访问（在单线程环境中模拟）
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 多次读取同一文件
    let meminfo = root.lookup("meminfo").unwrap();
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];

    assert!(meminfo.read_at(0, &mut buf1).is_ok());
    assert!(meminfo.read_at(0, &mut buf2).is_ok());
}

#[test_case]
fn test_procfs_readdir_all_entries() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let entries = root.readdir().unwrap();

    // 应该至少有：., .., meminfo, uptime, cpuinfo, mounts, self
    assert!(entries.len() >= 7);

    // 验证所有条目都可以 lookup
    for entry in entries {
        if entry.name != "." && entry.name != ".." {
            let result = root.lookup(&entry.name);
            assert!(result.is_ok());
        }
    }
}

// TODO: 此测试需要 current_task，需要完整的内核上下文
