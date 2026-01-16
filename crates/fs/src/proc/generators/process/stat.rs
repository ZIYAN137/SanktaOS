//! `/proc/[pid]/stat` 生成器

use alloc::{format, vec::Vec};

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// 为指定任务生成 `/proc/[pid]/stat` 内容的生成器
pub struct StatGenerator {
    pid: u32,
}

impl StatGenerator {
    /// 创建生成器（绑定到指定 pid）。
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl ContentGenerator for StatGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task = fs_ops().get_task(self.pid).ok_or(FsError::NotFound)?;

        let state_char = task.state().to_char();
        let name = task.name();

        // Linux /proc/[pid]/stat 格式（简化版）
        let content = format!(
            "{} ({}) {} {} {} {} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
            task.tid(),
            name,
            state_char,
            task.ppid(),
            task.pgid(),
            0, // session
        );

        Ok(content.into_bytes())
    }
}
