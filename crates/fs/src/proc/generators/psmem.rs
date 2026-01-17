//! /proc/psmem 生成器 - 进程内存快照

use alloc::{format, string::String, vec::Vec};

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// `/proc/psmem` 内容生成器。
pub struct PsmemGenerator;

impl ContentGenerator for PsmemGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let ops = fs_ops();
        let pids = ops.list_process_pids();

        let mut out = String::new();
        out.push_str(
            "PID\tVmSize(kB)\tVmRSS(kB)\tStack(kB)\tHeap+Data(kB)\tMmap(kB)\tExe(kB)\tName\n",
        );

        for pid in pids {
            let Some(task) = ops.get_task(pid) else {
                continue;
            };

            let name = task
                .exe_path()
                .unwrap_or_else(|| format!("task_{}", task.tid()));

            let (vm_size_kb, rss_kb, stack_kb, data_kb, mmap_kb, exe_kb) = task
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
                        s.mmap_bytes / 1024,
                        s.text_bytes / 1024,
                    )
                })
                .unwrap_or((0, 0, 0, 0, 0, 0));

            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                pid, vm_size_kb, rss_kb, stack_kb, data_kb, mmap_kb, exe_kb, name
            ));
        }

        Ok(out.into_bytes())
    }
}
