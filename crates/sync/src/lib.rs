//! 同步原语
//!
//! 向其它内核模块提供基本的锁和同步原语
//! 包括自旋锁、读写锁、中断保护等
//!
//! # 架构依赖
//!
//! 此 crate 通过 `ArchOps` trait 抽象架构相关操作。
//! 使用前必须调用 `register_arch_ops` 注册实现。

#![no_std]

mod intr_guard;
mod preempt;
mod raw_spin_lock;
mod raw_spin_lock_without_guard;
mod rwlock;
mod spin_lock;
mod ticket_lock;

pub use intr_guard::*;
pub use preempt::{PreemptGuard, preempt_disable, preempt_disabled, preempt_enable};
pub use raw_spin_lock::*;
pub use raw_spin_lock_without_guard::*;
pub use rwlock::*;
pub use spin_lock::*;
pub use ticket_lock::*;

use core::sync::atomic::{AtomicUsize, Ordering};

/// 架构相关操作的 trait
///
/// 由 os crate 实现并注册，提供中断控制和 CPU 信息
pub trait ArchOps: Send + Sync {
    /// 读取并禁用中断，返回之前的状态
    ///
    /// # Safety
    /// 调用者必须确保在适当的上下文中调用
    unsafe fn read_and_disable_interrupts(&self) -> usize;

    /// 恢复中断状态
    ///
    /// # Safety
    /// flags 必须是之前 read_and_disable_interrupts 返回的值
    unsafe fn restore_interrupts(&self, flags: usize);

    /// 获取 SSTATUS_SIE 常量（中断使能位）
    fn sstatus_sie(&self) -> usize;

    /// 获取当前 CPU ID
    fn cpu_id(&self) -> usize;

    /// 获取最大 CPU 数量
    fn max_cpu_count(&self) -> usize;
}

/// 全局架构操作实例（存储 fat pointer 的两个部分）
static ARCH_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static ARCH_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册架构操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_arch_ops(ops: &'static dyn ArchOps) {
    let ptr = ops as *const dyn ArchOps;
    // SAFETY: transmute 在这里是安全的，因为 fat pointer 的布局是 (data, vtable)
    let (data, vtable) = unsafe { core::mem::transmute::<*const dyn ArchOps, (usize, usize)>(ptr) };
    ARCH_OPS_DATA.store(data, Ordering::Release);
    ARCH_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取架构操作实例
#[inline]
pub(crate) fn arch_ops() -> &'static dyn ArchOps {
    let data = ARCH_OPS_DATA.load(Ordering::Acquire);
    let vtable = ARCH_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("sync: ArchOps not registered, call register_arch_ops first");
    }
    // SAFETY: data 和 vtable 是通过 register_arch_ops 设置的有效指针
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn ArchOps>((data, vtable)) }
}
