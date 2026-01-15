//! /proc/[pid]/maps 生成器

use alloc::{format, string::String, vec::Vec};

use crate::ops::fs_ops;
use crate::proc::ContentGenerator;
use vfs::FsError;

/// /proc/[pid]/maps 生成器
pub struct MapsGenerator {
    pid: u32,
}

impl MapsGenerator {
    pub fn new(pid: u32) -> Self {
        Self { pid }
    }
}

impl ContentGenerator for MapsGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let task = fs_ops().get_task(self.pid).ok_or(FsError::NotFound)?;

        let areas = task.memory_areas();
        if areas.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = String::new();
        for area in areas {
            let path = area.path.as_deref().unwrap_or("");
            out.push_str(&format!(
                "{:016x}-{:016x} {} {:08x} {} {:>8} {}\n",
                area.start, area.end, area.perm, area.offset, area.dev, area.inode, path
            ));
        }

        Ok(out.into_bytes())
    }
}
