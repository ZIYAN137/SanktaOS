//! FsOps trait 实现
//!
//! 此模块实现 fs crate 的 FsOps trait，桥接 os crate 的依赖。

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use ::fs::{FsOps, MemoryAreaInfo, MountInfo, TaskInfo, TaskState, VmStats};
use uapi::time::TimeSpec;

use crate::config::{EXT4_BLOCK_SIZE, FS_IMAGE_SIZE, PAGE_SIZE, VIRTIO_BLK_SECTOR_SIZE};
use crate::kernel::{TaskManagerTrait, TASK_MANAGER};
use crate::mm::frame_allocator::{get_free_frames, get_total_frames};
use crate::mm::AreaType;
use crate::time_ext::timespec_now;
use crate::vfs::{MountFlags, MOUNT_TABLE};

/// FsOps 实现
struct FsOpsImpl;

impl FsOps for FsOpsImpl {
    fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    fn ext4_block_size(&self) -> usize {
        EXT4_BLOCK_SIZE
    }

    fn fs_image_size(&self) -> usize {
        FS_IMAGE_SIZE
    }

    fn virtio_blk_sector_size(&self) -> usize {
        VIRTIO_BLK_SECTOR_SIZE
    }

    fn timespec_now(&self) -> TimeSpec {
        timespec_now()
    }

    fn get_task(&self, pid: u32) -> Option<Arc<dyn TaskInfo>> {
        let task = TASK_MANAGER.lock().get_task(pid)?;
        Some(Arc::new(TaskInfoWrapper { task }))
    }

    fn list_process_pids(&self) -> Vec<u32> {
        TASK_MANAGER.lock().list_process_pids_snapshot()
    }

    fn current_task_pid(&self) -> u32 {
        crate::kernel::current_task().lock().pid
    }

    fn get_uptime_ms(&self) -> u64 {
        crate::arch::timer::get_time_ms() as u64
    }

    fn get_total_frames(&self) -> usize {
        get_total_frames()
    }

    fn get_free_frames(&self) -> usize {
        get_free_frames()
    }

    fn proc_cpuinfo(&self) -> Vec<u8> {
        crate::arch::info::proc_cpuinfo()
    }

    fn list_mounts(&self) -> Vec<MountInfo> {
        MOUNT_TABLE
            .list_all()
            .into_iter()
            .map(|(path, mp)| MountInfo {
                device: mp.device.clone().unwrap_or_default(),
                path,
                fs_type: mp.fs.fs_type().to_string(),
                read_only: mp.flags.contains(MountFlags::READ_ONLY),
            })
            .collect()
    }
}

/// TaskInfo 包装器
struct TaskInfoWrapper {
    task: Arc<crate::sync::SpinLock<crate::kernel::TaskStruct>>,
}

// SAFETY: TaskInfoWrapper 内部的 task 是 Send + Sync 的
unsafe impl Send for TaskInfoWrapper {}
unsafe impl Sync for TaskInfoWrapper {}

impl TaskInfo for TaskInfoWrapper {
    fn pid(&self) -> u32 {
        self.task.lock().pid
    }

    fn tid(&self) -> u32 {
        self.task.lock().tid
    }

    fn ppid(&self) -> u32 {
        self.task.lock().ppid
    }

    fn pgid(&self) -> u32 {
        self.task.lock().pgid
    }

    fn name(&self) -> String {
        let task = self.task.lock();
        alloc::format!("task_{}", task.tid)
    }

    fn state(&self) -> TaskState {
        let task = self.task.lock();
        match task.state {
            crate::kernel::TaskState::Running => TaskState::Running,
            crate::kernel::TaskState::Interruptible => TaskState::Interruptible,
            crate::kernel::TaskState::Uninterruptible => TaskState::Uninterruptible,
            crate::kernel::TaskState::Stopped => TaskState::Stopped,
            crate::kernel::TaskState::Zombie => TaskState::Zombie,
        }
    }

    fn exe_path(&self) -> Option<String> {
        self.task.lock().exe_path.clone()
    }

    fn cmdline(&self) -> Vec<String> {
        // TODO: 从任务中获取真实的命令行参数
        Vec::new()
    }

    fn vm_stats(&self) -> Option<VmStats> {
        let task = self.task.lock();
        let ms = task.memory_space.as_ref()?;
        let ms = ms.lock();

        let mut stats = VmStats::default();

        for area in ms.areas().iter() {
            let at = area.area_type();
            let is_user = matches!(
                at,
                AreaType::UserText
                    | AreaType::UserRodata
                    | AreaType::UserData
                    | AreaType::UserBss
                    | AreaType::UserStack
                    | AreaType::UserHeap
                    | AreaType::UserMmap
            );
            if !is_user {
                continue;
            }

            let pages = area.vpn_range().len();
            let bytes = pages * PAGE_SIZE;

            let rss_pages = area.mapped_pages();
            stats.rss_bytes = stats.rss_bytes.saturating_add(rss_pages * PAGE_SIZE);

            match at {
                AreaType::UserText => stats.text_bytes = stats.text_bytes.saturating_add(bytes),
                AreaType::UserRodata => {
                    stats.rodata_bytes = stats.rodata_bytes.saturating_add(bytes)
                }
                AreaType::UserData => stats.data_bytes = stats.data_bytes.saturating_add(bytes),
                AreaType::UserBss => stats.bss_bytes = stats.bss_bytes.saturating_add(bytes),
                AreaType::UserHeap => stats.heap_bytes = stats.heap_bytes.saturating_add(bytes),
                AreaType::UserStack => stats.stack_bytes = stats.stack_bytes.saturating_add(bytes),
                AreaType::UserMmap => stats.mmap_bytes = stats.mmap_bytes.saturating_add(bytes),
                _ => {}
            }
        }

        Some(stats)
    }

    fn memory_areas(&self) -> Vec<MemoryAreaInfo> {
        use crate::mm::address::{PageNum, UsizeConvert};
        use crate::mm::page_table::UniversalPTEFlag;

        let task = self.task.lock();
        let Some(ms) = task.memory_space.as_ref() else {
            return Vec::new();
        };
        let ms = ms.lock();

        let mut areas: Vec<_> = ms
            .areas()
            .iter()
            .filter(|a| {
                matches!(
                    a.area_type(),
                    AreaType::UserText
                        | AreaType::UserRodata
                        | AreaType::UserData
                        | AreaType::UserBss
                        | AreaType::UserStack
                        | AreaType::UserHeap
                        | AreaType::UserMmap
                )
            })
            .collect();

        areas.sort_by_key(|a| a.vpn_range().start().start_addr().as_usize());

        areas
            .into_iter()
            .map(|a| {
                let start = a.vpn_range().start().start_addr().as_usize();
                let end = a.vpn_range().end().start_addr().as_usize();

                let perm = a.permission();
                let r = if perm.contains(UniversalPTEFlag::READABLE) {
                    'r'
                } else {
                    '-'
                };
                let w = if perm.contains(UniversalPTEFlag::WRITEABLE) {
                    'w'
                } else {
                    '-'
                };
                let x = if perm.contains(UniversalPTEFlag::EXECUTABLE) {
                    'x'
                } else {
                    '-'
                };

                let label = match a.area_type() {
                    AreaType::UserText => "[text]",
                    AreaType::UserRodata => "[rodata]",
                    AreaType::UserData => "[data]",
                    AreaType::UserBss => "[bss]",
                    AreaType::UserHeap => "[heap]",
                    AreaType::UserStack => "[stack]",
                    AreaType::UserMmap => "[mmap]",
                    _ => "[kernel]",
                };

                MemoryAreaInfo {
                    start,
                    end,
                    perm: alloc::format!("{}{}{}p", r, w, x),
                    offset: 0,
                    dev: "00:00".to_string(),
                    inode: 0,
                    path: Some(label.to_string()),
                }
            })
            .collect()
    }

    fn utime(&self) -> u64 {
        0 // TODO
    }

    fn stime(&self) -> u64 {
        0 // TODO
    }

    fn num_threads(&self) -> usize {
        1 // TODO
    }

    fn start_time(&self) -> u64 {
        0 // TODO
    }
}

static FS_OPS: FsOpsImpl = FsOpsImpl;

/// 初始化 FsOps
pub fn init() {
    // SAFETY: 在单线程环境下调用，且只调用一次
    unsafe {
        ::fs::register_fs_ops(&FS_OPS);
    }
}
