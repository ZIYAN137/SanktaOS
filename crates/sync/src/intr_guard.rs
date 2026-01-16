//! 中断保护器
//!
//! 基于 RAII 实现中断保护，在创建时禁用中断，销毁时恢复。
//!
//! 注意：禁用中断只能阻止**本地 CPU** 的“任务 vs 本地中断”并发，
//! 并不能阻止其他 CPU 的并行访问；多核共享数据仍需要配合自旋锁等原语。

use crate::arch_ops;
use core::ops::Drop;

/// 中断保护器，基于 RAII 实现中断保护。
///
/// 在创建时原子地禁用中断并保存之前的状态；
/// 在销毁时自动恢复之前的中断状态。
///
/// # 示例
/// ```ignore
/// {
///     let guard = IntrGuard::new(); // 禁用中断
///     // 临界区代码
/// } // 离开作用域，自动恢复中断状态
/// ```
pub struct IntrGuard {
    flags: usize,
}

impl IntrGuard {
    /// 原子地禁用中断并返回一个 IntrGuard 实例。
    ///
    /// 该实例在离开作用域时会自动恢复中断状态。
    pub fn new() -> Self {
        // SAFETY: 调用者必须确保在创建 IntrGuard 实例时，
        // 没有其他代码会修改中断状态，从而保证不可重入性。
        let flags = unsafe { arch_ops().read_and_disable_interrupts() };
        IntrGuard { flags }
    }

    /// 检查进入临界区前，中断是否处于启用状态。
    ///
    /// # 返回值
    /// 中断是否处于启用状态
    #[allow(dead_code)]
    pub fn was_enabled(&self) -> bool {
        self.flags & arch_ops().sstatus_sie() != 0
    }
}

impl Drop for IntrGuard {
    /// 当 IntrGuard 离开作用域时，自动恢复中断状态。
    fn drop(&mut self) {
        // SAFETY: flags 是在创建 IntrGuard 时保存的，
        // 因此恢复操作是安全的。
        unsafe { arch_ops().restore_interrupts(self.flags) };
    }
}
