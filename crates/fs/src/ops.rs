//! FS 运行时操作 trait 定义和注册
//!
//! 此模块定义了 FS 层需要的外部依赖接口，通过 trait 抽象实现与 os crate 的解耦。

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use uapi::time::TimeSpec;

/// FS 运行时操作
///
/// 此 trait 抽象了 FS 层需要的运行时操作，包括配置、时间和任务信息。
/// os crate 需要实现此 trait 并在启动时注册。
pub trait FsOps: Send + Sync {
    // ========== 配置 ==========

    /// 获取页大小
    fn page_size(&self) -> usize;

    /// 获取 Ext4 块大小
    fn ext4_block_size(&self) -> usize;

    /// 获取文件系统镜像大小
    fn fs_image_size(&self) -> usize;

    /// 获取 VirtIO 块设备扇区大小
    fn virtio_blk_sector_size(&self) -> usize;

    // ========== 时间 ==========

    /// 获取当前时间
    fn timespec_now(&self) -> TimeSpec;

    // ========== 任务管理（procfs 需要）==========

    /// 获取指定 PID 的任务信息
    fn get_task(&self, pid: u32) -> Option<Arc<dyn TaskInfo>>;

    /// 列出所有进程 PID
    fn list_process_pids(&self) -> Vec<u32>;

    /// 获取当前任务的 PID（用于 /proc/self）
    fn current_task_pid(&self) -> u32;

    // ========== 系统信息（procfs 需要）==========

    /// 获取系统运行时间（毫秒）
    fn get_uptime_ms(&self) -> u64;

    /// 获取总物理页帧数
    fn get_total_frames(&self) -> usize;

    /// 获取空闲物理页帧数
    fn get_free_frames(&self) -> usize;

    /// 获取 CPU 信息（/proc/cpuinfo 格式）
    fn proc_cpuinfo(&self) -> Vec<u8>;

    /// 获取挂载点列表
    fn list_mounts(&self) -> Vec<MountInfo>;
}

/// 挂载点信息（用于 /proc/mounts）
#[derive(Clone)]
pub struct MountInfo {
    /// 设备名称
    pub device: String,
    /// 挂载路径
    pub path: String,
    /// 文件系统类型
    pub fs_type: String,
    /// 是否只读
    pub read_only: bool,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 运行中
    Running,
    /// 可中断睡眠
    Interruptible,
    /// 不可中断睡眠
    Uninterruptible,
    /// 已停止
    Stopped,
    /// 僵尸
    Zombie,
}

impl TaskState {
    /// 转换为状态字符
    pub fn to_char(self) -> char {
        match self {
            TaskState::Running => 'R',
            TaskState::Interruptible => 'S',
            TaskState::Uninterruptible => 'D',
            TaskState::Stopped => 'T',
            TaskState::Zombie => 'Z',
        }
    }

    /// 获取状态名称
    pub fn name(self) -> &'static str {
        match self {
            TaskState::Running => "running",
            TaskState::Interruptible => "sleeping",
            TaskState::Uninterruptible => "disk sleep",
            TaskState::Stopped => "stopped",
            TaskState::Zombie => "zombie",
        }
    }
}

/// 任务信息接口（用于 procfs）
pub trait TaskInfo: Send + Sync {
    /// 获取进程 ID
    fn pid(&self) -> u32;

    /// 获取线程 ID
    fn tid(&self) -> u32;

    /// 获取父进程 ID
    fn ppid(&self) -> u32;

    /// 获取进程组 ID
    fn pgid(&self) -> u32;

    /// 获取进程名称
    fn name(&self) -> String;

    /// 获取进程状态
    fn state(&self) -> TaskState;

    /// 获取可执行文件路径
    fn exe_path(&self) -> Option<String>;

    /// 获取命令行参数
    fn cmdline(&self) -> Vec<String>;

    /// 获取虚拟内存统计信息
    fn vm_stats(&self) -> Option<VmStats>;

    /// 获取内存区域信息（用于 /proc/[pid]/maps）
    fn memory_areas(&self) -> Vec<MemoryAreaInfo>;

    /// 获取用户态 CPU 时间（时钟滴答数）
    fn utime(&self) -> u64;

    /// 获取内核态 CPU 时间（时钟滴答数）
    fn stime(&self) -> u64;

    /// 获取线程数
    fn num_threads(&self) -> usize;

    /// 获取启动时间（时钟滴答数）
    fn start_time(&self) -> u64;
}

/// 虚拟内存统计信息
#[derive(Clone, Default)]
pub struct VmStats {
    /// 代码段大小（字节）
    pub text_bytes: usize,
    /// 只读数据段大小（字节）
    pub rodata_bytes: usize,
    /// 数据段大小（字节）
    pub data_bytes: usize,
    /// BSS 段大小（字节）
    pub bss_bytes: usize,
    /// 堆大小（字节）
    pub heap_bytes: usize,
    /// 栈大小（字节）
    pub stack_bytes: usize,
    /// mmap 区域大小（字节）
    pub mmap_bytes: usize,
    /// 常驻内存大小（字节）
    pub rss_bytes: usize,
}

impl VmStats {
    /// 获取虚拟内存大小（KB）
    pub fn vm_size_kb(&self) -> usize {
        (self.text_bytes
            + self.rodata_bytes
            + self.data_bytes
            + self.bss_bytes
            + self.heap_bytes
            + self.stack_bytes
            + self.mmap_bytes)
            / 1024
    }

    /// 获取常驻内存大小（KB）
    pub fn rss_kb(&self) -> usize {
        self.rss_bytes / 1024
    }
}

/// 内存区域信息（用于 procfs /proc/[pid]/maps）
#[derive(Clone)]
pub struct MemoryAreaInfo {
    /// 起始地址
    pub start: usize,
    /// 结束地址
    pub end: usize,
    /// 权限字符串（如 "r-xp"）
    pub perm: String,
    /// 文件偏移
    pub offset: usize,
    /// 设备号
    pub dev: String,
    /// inode 号
    pub inode: usize,
    /// 映射路径
    pub path: Option<String>,
}

// ========== FsOps 注册 ==========

static FS_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static FS_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册 FS 操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_fs_ops(ops: &'static dyn FsOps) {
    let ptr = ops as *const dyn FsOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) =
        unsafe { core::mem::transmute::<*const dyn FsOps, (usize, usize)>(ptr) };
    FS_OPS_DATA.store(data, Ordering::Release);
    FS_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的 FS 操作实现
///
/// # Panics
/// 如果尚未调用 [`register_fs_ops`] 注册实现，则 panic
#[inline]
pub fn fs_ops() -> &'static dyn FsOps {
    let data = FS_OPS_DATA.load(Ordering::Acquire);
    let vtable = FS_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("fs: FsOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn FsOps>((data, vtable)) }
}
