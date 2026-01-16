use super::*;
use crate::vfs::{FileSystem, InodeType};
use crate::println;
use alloc::vec;
use alloc::vec::Vec;

// P0 核心功能测试

#[test_case]
fn test_ext4_create_directory() {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 调试：检查文件系统状态
    if let Ok(statfs) = fs.statfs() {
        println!(
            "Free blocks: {}/{}",
            statfs.free_blocks, statfs.total_blocks
        );
        println!(
            "Free inodes: {}/{}",
            statfs.free_inodes, statfs.total_inodes
        );
    }

    // 创建目录
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    if let Err(e) = result {
        println!("{}", e.to_errno());
    }
    assert!(result.is_ok());

    if let Ok(_) = result {
        // 验证目录存在
        let lookup_result = root.lookup("testdir");
        assert!(lookup_result.is_ok());
        if let Ok(dir_inode) = lookup_result {
            let metadata = dir_inode.metadata();
            if let Ok(metadata) = metadata {
                assert!(metadata.inode_type == InodeType::Directory);
            }
        }
    }
}

#[test_case]
fn test_ext4_readdir() {
    // 创建 Ext4 文件系统并添加文件
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建几个文件
    let _ = root.create("file1.txt", FileMode::from_bits_truncate(0o644));
    let _ = root.create("file2.txt", FileMode::from_bits_truncate(0o644));
    let _ = root.mkdir("dir1", FileMode::from_bits_truncate(0o755));

    // 列出目录内容
    let entries_result = root.readdir();
    assert!(entries_result.is_ok());

    if let Ok(entries) = entries_result {
        assert!(entries.len() >= 3); // 至少包含我们创建的 3 个项

        // 验证包含我们创建的项
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"file2.txt"));
        assert!(names.contains(&"dir1"));
    }
}

#[test_case]
fn test_ext4_nested_directory() {
    // 创建嵌套目录结构
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建第一级目录
    let dir1_res = root.mkdir("dir1", FileMode::from_bits_truncate(0o755));
    assert!(dir1_res.is_ok());

    if let Ok(dir1) = dir1_res {
        // 在子目录中创建文件
        let result = dir1.create("file.txt", FileMode::from_bits_truncate(0o644));
        assert!(result.is_ok());

        // 验证文件存在
        let lookup_result = dir1.lookup("file.txt");
        assert!(lookup_result.is_ok());
    }
}

#[test_case]
fn test_ext4_lookup_in_directory() {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建目录和文件
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    let create_result = dir.create("file.txt", FileMode::from_bits_truncate(0o644));
    assert!(create_result.is_ok());

    // 在目录中查找文件
    let result = dir.lookup("file.txt");
    assert!(result.is_ok());
}

// P1 重要功能测试

#[test_case]
fn test_ext4_unlink_directory() {
    // 创建 Ext4 文件系统和空目录
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let _ = root.mkdir("emptydir", FileMode::from_bits_truncate(0o755));

    // 删除空目录
    let result = root.unlink("emptydir");
    assert!(result.is_ok());

    // 验证目录不存在
    let lookup_result = root.lookup("emptydir");
    assert!(lookup_result.is_err());
}

#[test_case]
fn test_ext4_readdir_empty() {
    // 创建空目录
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let dir_res = root.mkdir("emptydir", FileMode::from_bits_truncate(0o755));
    assert!(dir_res.is_ok());

    if let Ok(dir) = dir_res {
        // 读取空目录
        let entries_res = dir.readdir();
        assert!(entries_res.is_ok());
        if let Ok(entries) = entries_res {
            assert!(entries.is_empty() || entries.len() <= 2); // 可能包含 . 和 ..
        }
    }
}

#[test_case]
fn test_ext4_directory_metadata() {
    // 创建目录
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let dir_res = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    if let Err(e) = dir_res {
        println!("{}", e.to_errno());
    }
    assert!(dir_res.is_ok());

    if let Ok(dir) = dir_res {
        // 获取元数据
        let metadata_res = dir.metadata();
        assert!(metadata_res.is_ok());
        if let Ok(metadata) = metadata_res {
            assert!(metadata.inode_type == InodeType::Directory);
            assert!(metadata.mode.can_read());
            assert!(metadata.mode.can_write());
            assert!(metadata.mode.can_execute()); // 目录需要执行权限才能进入
        }
    }
}

// P2 边界和错误处理测试

#[test_case]
fn test_ext4_mkdir_duplicate() {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 第一次创建
    let _ = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));

    // 第二次创建同名目录应该失败
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    assert!(result.is_err());
    assert!(matches!(result, Err(FsError::AlreadyExists)));
}

#[test_case]
fn test_ext4_write_to_directory() {
    // 创建目录
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let dir_res = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    assert!(dir_res.is_ok());

    if let Ok(dir) = dir_res {
        // 尝试写入目录（应该失败）
        let result = dir.write_at(0, b"test");
        assert!(result.is_err());
        assert!(matches!(result, Err(FsError::IsDirectory)));
    }
}

#[test_case]
fn test_ext4_read_from_directory() {
    // 创建目录
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let dir_res = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    assert!(dir_res.is_ok());

    if let Ok(dir) = dir_res {
        // 尝试读取目录（应该失败）
        let mut buf = vec![0u8; 10];
        let result = dir.read_at(0, &mut buf);
        assert!(result.is_err());
        assert!(matches!(result, Err(FsError::IsDirectory)));
    }
}

#[test_case]
fn test_ext4_lookup_in_file() {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试在文件中查找（应该失败）
    let result = inode.lookup("anything");
    assert!(result.is_err());
    assert!(matches!(result, Err(FsError::NotDirectory)));
}
