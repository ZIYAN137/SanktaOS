//! /proc/mounts 生成器

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ops::fs_ops;
use crate::proc::inode::ContentGenerator;
use vfs::FsError;

pub struct MountsGenerator;

impl ContentGenerator for MountsGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let mut content = String::new();

        let mounts = fs_ops().list_mounts();

        for mount in mounts {
            let device = if mount.device.is_empty() {
                "none"
            } else {
                &mount.device
            };

            let mut options = Vec::new();
            if mount.read_only {
                options.push("ro");
            } else {
                options.push("rw");
            }
            options.push("relatime");

            let line = format!(
                "{} {} {} {} 0 0\n",
                device,
                mount.path,
                mount.fs_type,
                options.join(",")
            );

            content.push_str(&line);
        }

        Ok(content.into_bytes())
    }
}
