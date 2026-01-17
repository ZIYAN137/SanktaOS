//! `/proc/[pid]/cmdline` 生成器

use alloc::vec::Vec;

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// 为指定任务生成 `/proc/[pid]/cmdline` 内容的生成器
pub struct CmdlineGenerator {
    pid: u32,
}

impl CmdlineGenerator {
    /// 创建生成器（绑定到指定 pid）。
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl ContentGenerator for CmdlineGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task = fs_ops().get_task(self.pid).ok_or(FsError::NotFound)?;

        let cmdline = task.cmdline();
        if cmdline.is_empty() {
            // 简化实现：返回进程名
            let mut content = task.name().into_bytes();
            content.push(0);
            return Ok(content);
        }

        // Linux cmdline 格式: 参数之间用 \0 分隔
        let mut content = Vec::new();
        for arg in cmdline {
            content.extend_from_slice(arg.as_bytes());
            content.push(0);
        }

        Ok(content)
    }
}
