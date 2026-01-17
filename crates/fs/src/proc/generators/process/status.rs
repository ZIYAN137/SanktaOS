//! `/proc/[pid]/status` 生成器

use alloc::{format, vec::Vec};

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// 为指定任务生成 `/proc/[pid]/status` 内容的生成器
pub struct StatusGenerator {
    pid: u32,
}

impl StatusGenerator {
    /// 创建生成器（绑定到指定 pid）。
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl ContentGenerator for StatusGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task = fs_ops().get_task(self.pid).ok_or(FsError::NotFound)?;

        let state = task.state();
        let state_char = state.to_char();
        let name = task.name();

        let (vm_size_kb, rss_kb, stack_kb, data_kb, exe_kb, mmap_kb) = task
            .vm_stats()
            .map(|s| {
                let data_bytes = s
                    .data_bytes
                    .saturating_add(s.bss_bytes)
                    .saturating_add(s.heap_bytes);
                (
                    s.vm_size_kb(),
                    s.rss_kb(),
                    s.stack_bytes / 1024,
                    data_bytes / 1024,
                    s.text_bytes / 1024,
                    s.mmap_bytes / 1024,
                )
            })
            .unwrap_or((0, 0, 0, 0, 0, 0));

        let content = format!(
            "Name:\t{}\n\
             State:\t{} ({})\n\
             Tgid:\t{}\n\
             Pid:\t{}\n\
             PPid:\t{}\n\
             TracerPid:\t0\n\
             Uid:\t0\t0\t0\t0\n\
             Gid:\t0\t0\t0\t0\n\
             VmSize:\t{:>8} kB\n\
             VmRSS:\t{:>8} kB\n\
             VmStk:\t{:>8} kB\n\
             VmData:\t{:>8} kB\n\
             VmExe:\t{:>8} kB\n\
             VmLib:\t{:>8} kB\n\
             VmPTE:\t{:>8} kB\n\
             VmSwap:\t{:>8} kB\n\
             VmMmap:\t{:>8} kB\n",
            name,
            state_char,
            state.name(),
            task.pid(),
            task.tid(),
            task.ppid(),
            vm_size_kb,
            rss_kb,
            stack_kb,
            data_kb,
            exe_kb,
            0usize, // VmLib
            0usize, // VmPTE
            0usize, // VmSwap
            mmap_kb,
        );

        Ok(content.into_bytes())
    }
}
