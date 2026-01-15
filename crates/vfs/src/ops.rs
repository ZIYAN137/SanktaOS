//! VFS 运行时操作 trait 定义和注册
//!
//! 此模块定义了 VFS 层需要的外部依赖接口，通过 trait 抽象实现与 os crate 的解耦。

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use uapi::time::TimeSpec;

use crate::Dentry;

/// VFS 运行时操作
///
/// 此 trait 抽象了 VFS 层需要的运行时操作，包括任务上下文、配置、时间和控制台操作。
/// os crate 需要实现此 trait 并在启动时注册。
pub trait VfsOps: Send + Sync {
    // ========== 任务上下文 ==========

    /// 获取当前任务的工作目录
    fn current_cwd(&self) -> Option<Arc<Dentry>>;

    /// 获取当前任务的根目录
    fn current_root(&self) -> Option<Arc<Dentry>>;

    // ========== 配置 ==========

    /// 获取默认最大文件描述符数
    fn default_max_fds(&self) -> usize;

    // ========== 时间 ==========

    /// 获取当前时间
    fn timespec_now(&self) -> TimeSpec;

    // ========== 用户空间访问保护 ==========

    /// 进入用户空间访问模式（替代 SumGuard::new()）
    fn enter_user_access(&self);

    /// 退出用户空间访问模式
    fn exit_user_access(&self);

    // ========== 控制台操作 ==========

    /// 从控制台读取一个字符
    fn console_getchar(&self) -> Option<u8>;

    /// 向控制台输出一个字符
    fn console_putchar(&self, c: u8);

    /// 向控制台输出字符串
    fn console_write_str(&self, s: &str);
}

/// 字符设备驱动接口
///
/// 用于 char_dev_file.rs 中的字符设备文件实现
pub trait CharDriver: Send + Sync {
    /// 尝试读取一个字符（非阻塞）
    fn try_read(&self) -> Option<u8>;

    /// 写入数据
    fn write(&self, data: &[u8]);

    /// 执行 ioctl 操作
    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, i32>;
}

/// 设备操作
///
/// 此 trait 抽象了设备驱动相关的操作
pub trait DeviceOps: Send + Sync {
    /// 获取字符设备驱动
    fn get_chrdev_driver(&self, dev: u64) -> Option<Arc<dyn CharDriver>>;

    /// 获取块设备索引
    fn get_blkdev_index(&self, dev: u64) -> Option<usize>;

    /// 读取块设备数据
    fn read_block(&self, idx: usize, block_id: usize, buf: &mut [u8]) -> bool;

    /// 写入块设备数据
    fn write_block(&self, idx: usize, block_id: usize, buf: &[u8]) -> bool;

    /// 获取块设备总块数
    fn blkdev_total_blocks(&self, idx: usize) -> usize;
}

// ========== VfsOps 注册 ==========

static VFS_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static VFS_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册 VFS 操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_vfs_ops(ops: &'static dyn VfsOps) {
    let ptr = ops as *const dyn VfsOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) =
        unsafe { core::mem::transmute::<*const dyn VfsOps, (usize, usize)>(ptr) };
    VFS_OPS_DATA.store(data, Ordering::Release);
    VFS_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的 VFS 操作实现
///
/// # Panics
/// 如果尚未调用 [`register_vfs_ops`] 注册实现，则 panic
#[inline]
pub fn vfs_ops() -> &'static dyn VfsOps {
    let data = VFS_OPS_DATA.load(Ordering::Acquire);
    let vtable = VFS_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("vfs: VfsOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn VfsOps>((data, vtable)) }
}

// ========== DeviceOps 注册 ==========

static DEVICE_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static DEVICE_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册设备操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_device_ops(ops: &'static dyn DeviceOps) {
    let ptr = ops as *const dyn DeviceOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) =
        unsafe { core::mem::transmute::<*const dyn DeviceOps, (usize, usize)>(ptr) };
    DEVICE_OPS_DATA.store(data, Ordering::Release);
    DEVICE_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的设备操作实现
///
/// # Panics
/// 如果尚未调用 [`register_device_ops`] 注册实现，则 panic
#[inline]
pub fn device_ops() -> &'static dyn DeviceOps {
    let data = DEVICE_OPS_DATA.load(Ordering::Acquire);
    let vtable = DEVICE_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("vfs: DeviceOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn DeviceOps>((data, vtable)) }
}

/// 用户空间访问保护 guard
///
/// 在作用域结束时自动退出用户空间访问模式
pub struct UserAccessGuard;

impl UserAccessGuard {
    /// 创建新的用户空间访问保护
    #[inline]
    pub fn new() -> Self {
        vfs_ops().enter_user_access();
        Self
    }
}

impl Drop for UserAccessGuard {
    #[inline]
    fn drop(&mut self) {
        vfs_ops().exit_user_access();
    }
}

impl Default for UserAccessGuard {
    fn default() -> Self {
        Self::new()
    }
}
