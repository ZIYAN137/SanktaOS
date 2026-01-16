use crate::device::block::RamDisk;
use crate::fs::TmpFs;
use crate::vfs::*;
use alloc::string::String;
use alloc::sync::Arc;

// ========== Test Assertions ==========
//
// These tests run inside the kernel test framework, where we prefer "record a failure and keep
// running" semantics (instead of panicking). Map familiar `assert!*` names to `kassert!` here,
// scoped only to `os::vfs::tests::*`.
#[cfg(test)]
macro_rules! assert {
    ($cond:expr $(,)?) => {
        $crate::kassert!($cond)
    };
    ($cond:expr, $($arg:tt)+) => {
        // Keep the failure recording behavior; ignore the message for now.
        $crate::kassert!($cond)
    };
}

#[cfg(test)]
macro_rules! assert_eq {
    ($left:expr, $right:expr $(,)?) => {{
        let left = &$left;
        let right = &$right;
        $crate::kassert!(left == right);
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left = &$left;
        let right = &$right;
        $crate::kassert!(left == right);
    }};
}

#[cfg(test)]
macro_rules! assert_ne {
    ($left:expr, $right:expr $(,)?) => {{
        let left = &$left;
        let right = &$right;
        $crate::kassert!(left != right);
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left = &$left;
        let right = &$right;
        $crate::kassert!(left != right);
    }};
}

// 测试辅助函数 (fixtures)

/// 创建一个测试用文件系统实例
///
/// 当前使用 TmpFs 作为测试文件系统（simple_fs 已移除）。
pub fn create_test_fs() -> Arc<dyn FileSystem> {
    TmpFs::new(0) as Arc<dyn FileSystem>
}

/// 创建一个指定大小的测试用 RamDisk
pub fn create_test_ramdisk(size_in_blocks: usize) -> Arc<RamDisk> {
    let block_size = 512;
    let total_size = size_in_blocks * block_size;
    RamDisk::new(total_size, block_size, 0)
}

/// 在测试文件系统中创建一个文件并写入内容
pub fn create_test_file_with_content(
    fs: &Arc<dyn FileSystem>,
    path: &str,
    content: &[u8],
) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    let inode = root.create(path, FileMode::from_bits_truncate(0o644))?;
    inode.write_at(0, content)?;
    Ok(inode)
}

/// 在测试文件系统中创建一个目录
pub fn create_test_dir(fs: &Arc<dyn FileSystem>, path: &str) -> Result<Arc<dyn Inode>, FsError> {
    let root = fs.root_inode();
    root.mkdir(path, FileMode::from_bits_truncate(0o755))
}

/// 从 Inode 创建一个 Dentry (用于测试)
pub fn create_test_dentry(name: &str, inode: Arc<dyn Inode>) -> Arc<Dentry> {
    Dentry::new(String::from(name), inode)
}

/// 创建一个测试用的 File 对象
pub fn create_test_file(name: &str, inode: Arc<dyn Inode>, flags: OpenFlags) -> Arc<dyn File> {
    let dentry = create_test_dentry(name, inode);
    Arc::new(RegFile::new(dentry, flags))
}

pub mod blk_dev_file;
pub mod char_dev_file;
pub mod dentry;
pub mod devno;
pub mod fd_table;
pub mod file;
pub mod mount;
pub mod path;
pub mod pipe;
pub mod stdio;
pub mod trait_file;
