//! Procfs 文件系统实现

use alloc::sync::Arc;

use crate::ops::fs_ops;
use crate::proc::ProcInode;
use vfs::{FileMode, FileSystem, FsError, Inode, StatFs};

/// ProcFS 文件系统对象（提供 `/proc` 目录树）。
pub struct ProcFS {
    root_inode: Arc<ProcInode>,
}

impl ProcFS {
    /// 创建新的 ProcFS 实例
    pub fn new() -> Arc<Self> {
        let root = ProcInode::new_proc_root_directory(FileMode::from_bits_truncate(
            0o555 | FileMode::S_IFDIR.bits(),
        ));

        Arc::new(Self { root_inode: root })
    }

    /// 初始化 proc 文件系统树结构
    pub fn init_tree(self: &Arc<Self>) -> Result<(), FsError> {
        use crate::proc::generators::{
            CpuinfoGenerator, MeminfoGenerator, MountsGenerator, PsmemGenerator, UptimeGenerator,
        };

        let root = &self.root_inode;

        // 创建 /proc/meminfo
        let meminfo = ProcInode::new_dynamic_file(
            "meminfo",
            Arc::new(MeminfoGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("meminfo", meminfo)?;

        // 创建 /proc/uptime
        let uptime = ProcInode::new_dynamic_file(
            "uptime",
            Arc::new(UptimeGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("uptime", uptime)?;

        // 创建 /proc/cpuinfo
        let cpuinfo = ProcInode::new_dynamic_file(
            "cpuinfo",
            Arc::new(CpuinfoGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("cpuinfo", cpuinfo)?;

        // 创建 /proc/mounts
        let mounts = ProcInode::new_dynamic_file(
            "mounts",
            Arc::new(MountsGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("mounts", mounts)?;

        // 创建 /proc/psmem
        let psmem = ProcInode::new_dynamic_file(
            "psmem",
            Arc::new(PsmemGenerator),
            FileMode::from_bits_truncate(0o444),
        );
        root.add_child("psmem", psmem)?;

        // 创建 /proc/self - 动态符号链接，指向当前进程
        let self_link = ProcInode::new_dynamic_symlink("self", || {
            use alloc::string::ToString;
            fs_ops().current_task_pid().to_string()
        });
        root.add_child("self", self_link)?;

        Ok(())
    }
}

impl FileSystem for ProcFS {
    fn fs_type(&self) -> &'static str {
        "proc"
    }

    fn root_inode(&self) -> Arc<dyn Inode> {
        self.root_inode.clone()
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn statfs(&self) -> Result<StatFs, FsError> {
        Ok(StatFs {
            block_size: 4096,
            total_blocks: 0,
            free_blocks: 0,
            available_blocks: 0,
            total_inodes: 0,
            free_inodes: 0,
            fsid: 0,
            max_filename_len: 255,
        })
    }
}
