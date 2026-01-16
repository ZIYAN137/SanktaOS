//! mmap 文件映射信息

use crate::MmFile;
use alloc::sync::Arc;
use uapi::mm::{MapFlags, ProtFlags};

/// 文件映射信息
pub struct MmapFile {
    /// 文件对象引用（用于权限检查和获取 Inode）
    pub file: Arc<dyn MmFile>,
    /// 文件偏移量（字节）
    pub offset: usize,
    /// 映射长度（字节）
    pub len: usize,
    /// 保护标志
    pub prot: ProtFlags,
    /// 映射标志
    pub flags: MapFlags,
}

// 手动实现 Debug，因为 dyn MmFile 没有实现 Debug
impl core::fmt::Debug for MmapFile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MmapFile")
            .field("file", &"<dyn MmFile>")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .field("prot", &self.prot)
            .field("flags", &self.flags)
            .finish()
    }
}
