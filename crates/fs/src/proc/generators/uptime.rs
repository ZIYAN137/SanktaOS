//! /proc/uptime 生成器

use alloc::format;
use alloc::vec::Vec;

use crate::ops::fs_ops;
use crate::proc::inode::ContentGenerator;
use vfs::FsError;

/// `/proc/uptime` 内容生成器。
pub struct UptimeGenerator;

impl ContentGenerator for UptimeGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let uptime_ms = fs_ops().get_uptime_ms();
        let uptime_sec = uptime_ms / 1000;
        let uptime_frac = (uptime_ms % 1000) / 10;

        // TODO: 获取空闲时间
        let idle_sec = 0;
        let idle_frac = 0;

        let content = format!(
            "{}.{:02} {}.{:02}\n",
            uptime_sec, uptime_frac, idle_sec, idle_frac
        );

        Ok(content.into_bytes())
    }
}
