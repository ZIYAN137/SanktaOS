use super::*;
use crate::vfs::FileSystem;
use alloc::string::String;

// P1 重要功能测试

#[test_case]
fn test_mount_fs() {
    // 创建文件系统
    let fs = create_test_fs();

    // 挂载到 /test
    let result = MOUNT_TABLE.mount(
        fs.clone(),
        "/test",
        MountFlags::empty(),
        Some(String::from("testfs")),
    );
    assert!(result.is_ok());

    // 查找挂载点
    let mount = MOUNT_TABLE.find_mount("/test");
    assert!(mount.is_some());
}

#[test_case]
fn test_mount_list() {
    // 挂载文件系统
    let fs1 = create_test_fs();

    MOUNT_TABLE
        .mount(fs1, "/mnt_test", MountFlags::empty(), None)
        .ok();

    // 列出挂载点
    let mounts = MOUNT_TABLE.list_mounts();
    // 至少应该有根文件系统
    assert!(mounts.len() >= 1);
}

// P2 边界和错误处理测试

#[test_case]
fn test_umount_fs() {
    // 创建文件系统并挂载
    let fs = create_test_fs();
    MOUNT_TABLE
        .mount(fs, "/test_umount2", MountFlags::empty(), None)
        .ok();

    // 卸载
    let result = MOUNT_TABLE.umount("/test_umount2");
    assert!(result.is_ok());

    // 卸载后应该找不到原挂载点（可能会匹配到根挂载点，但不应该是 /test_umount2）
    let mount = MOUNT_TABLE.find_mount("/test_umount2");
    if let Some(m) = mount {
        assert!(m.mount_path != "/test_umount2");
    }
    // 如果没有根挂载点，应该返回 None
    // 如果有根挂载点，应该返回根而不是 /test_umount2
}

// P3 Overmount 测试

#[test_case]
fn test_overmount() {
    // 创建两个文件系统
    let fs1 = create_test_fs();
    let fs2 = create_test_fs();

    // 在同一路径挂载两次
    let result1 = MOUNT_TABLE.mount(fs1, "/overmount_test", MountFlags::empty(), None);
    assert!(result1.is_ok());

    let result2 = MOUNT_TABLE.mount(fs2, "/overmount_test", MountFlags::empty(), None);
    assert!(result2.is_ok()); // 应该支持 overmount

    // 查找挂载点应该返回最新的
    let mount = MOUNT_TABLE.find_mount("/overmount_test");
    assert!(mount.is_some());

    // 卸载一次，应该还能找到挂载点（下层的）
    let umount_result = MOUNT_TABLE.umount("/overmount_test");
    assert!(umount_result.is_ok());

    let mount_after = MOUNT_TABLE.find_mount("/overmount_test");
    assert!(mount_after.is_some()); // 应该还有下层挂载

    // 再卸载一次，这次应该彻底没有了
    let umount_result2 = MOUNT_TABLE.umount("/overmount_test");
    assert!(umount_result2.is_ok());

    let mount_final = MOUNT_TABLE.find_mount("/overmount_test");
    if let Some(m) = mount_final {
        assert!(m.mount_path != "/overmount_test");
    }
}

#[test_case]
fn test_umount_root_should_fail() {
    // 尝试卸载根文件系统应该失败
    let result = MOUNT_TABLE.umount("/");
    assert!(result.is_err());
}

// P4 跨文件系统 lookup 测试

#[test_case]
fn test_lookup_across_mount_point() {
    // 让测试可重复执行：如果该挂载点曾经残留，先尽力卸载干净。
    while MOUNT_TABLE.umount("/mnt_lookup_test").is_ok() {}

    // 创建一个测试文件系统
    let fs = create_test_fs();

    // 在文件系统内创建一个文件
    let root = fs.root_inode();
    let create_result = root.create("testfile", FileMode::from_bits_truncate(0o644));
    assert!(create_result.is_ok());

    // 挂载到 /mnt_lookup_test
    let mount_result = MOUNT_TABLE.mount(fs.clone(), "/mnt_lookup_test", MountFlags::empty(), None);
    assert!(mount_result.is_ok());

    // 获取挂载点
    let mount_point = MOUNT_TABLE.find_mount("/mnt_lookup_test");
    assert!(mount_point.is_some());

    if let Some(mp) = mount_point {
        // 从挂载点的根开始查找文件
        let lookup_result = vfs_lookup_from(mp.root.clone(), "testfile");
        assert!(lookup_result.is_ok());
    }

    // 清理
    while MOUNT_TABLE.umount("/mnt_lookup_test").is_ok() {}
}
