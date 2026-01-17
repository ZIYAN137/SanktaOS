//! /proc/meminfo 生成器

use alloc::{format, vec::Vec};

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// `/proc/meminfo` 内容生成器。
pub struct MeminfoGenerator;

impl ContentGenerator for MeminfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let ops = fs_ops();
        let page_size = ops.page_size();
        let total_frames = ops.get_total_frames();
        let free_frames = ops.get_free_frames();

        let total_kb = (total_frames * page_size) / 1024;
        let free_kb = (free_frames * page_size) / 1024;
        let available_kb = free_kb;

        let content = format!(
            "MemTotal:       {:>8} kB
MemFree:        {:>8} kB
MemAvailable:   {:>8} kB
Buffers:        {:>8} kB
Cached:         {:>8} kB
SwapCached:     {:>8} kB
Active:         {:>8} kB
Inactive:       {:>8} kB
Active(anon):   {:>8} kB
Inactive(anon): {:>8} kB
Active(file):   {:>8} kB
Inactive(file): {:>8} kB
Unevictable:    {:>8} kB
Mlocked:        {:>8} kB
SwapTotal:      {:>8} kB
SwapFree:       {:>8} kB
Dirty:          {:>8} kB
Writeback:      {:>8} kB
AnonPages:      {:>8} kB
Mapped:         {:>8} kB
Shmem:          {:>8} kB
",
            total_kb, free_kb, available_kb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        );

        Ok(content.into_bytes())
    }
}
