//! ProcFS 符号链接测试

use super::*;

#[test_case]
fn test_procfs_self_symlink_exists() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let self_link = root.lookup("self");
    assert!(self_link.is_ok());

    let inode = self_link.unwrap();
    let metadata = inode.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Symlink);
}

// TODO: 此测试需要 current_task，需要完整的内核上下文
//

#[test_case]
fn test_procfs_symlink_metadata() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let self_link = root.lookup("self").unwrap();

    let metadata = self_link.metadata().unwrap();
    assert!(metadata.inode_type == InodeType::Symlink);
    assert!(metadata.nlinks == 1);
}

#[test_case]
fn test_procfs_symlink_not_writable() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let self_link = root.lookup("self").unwrap();

    // 符号链接不应该支持写入
    let result = self_link.write_at(0, b"test");
    assert!(result.is_err());
}

#[test_case]
fn test_procfs_symlink_not_readable_as_file() {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();
    let self_link = root.lookup("self").unwrap();

    // 符号链接不应该支持 read_at（应使用 read_link）
    let mut buf = [0u8; 10];
    let result = self_link.read_at(0, &mut buf);
    assert!(result.is_err());
}
