//! /proc/cpuinfo 生成器

use alloc::vec::Vec;

use crate::ops::fs_ops;
use crate::proc::inode::ContentGenerator;
use vfs::FsError;

/// `/proc/cpuinfo` 内容生成器。
pub struct CpuinfoGenerator;

impl ContentGenerator for CpuinfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        Ok(fs_ops().proc_cpuinfo())
    }
}
