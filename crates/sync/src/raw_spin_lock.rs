//! 自旋锁实现
//!
//! 基于原子操作实现自旋锁机制，结合 IntrGuard 实现中断保护。

use crate::intr_guard::IntrGuard;
use core::{
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

/// 自旋锁结构体，提供互斥访问临界区的能力。
///
/// 基于原子操作实现自旋锁机制，结合 IntrGuard 实现中断保护。
/// 不可重入 (即不能嵌套调用 RawSpinLock::lock())。
///
/// # 示例
/// ```ignore
/// let lock = RawSpinLock::new();
/// {
///   let guard = lock.lock(); // 获取锁，禁用中断
///   // 临界区代码
/// } // 离开作用域，自动释放锁并恢复中断状态
/// ```
#[derive(Debug)]
pub struct RawSpinLock {
    lock: AtomicBool,
}

impl RawSpinLock {
    /// 创建一个新的 RawSpinLock 实例。
    pub const fn new() -> Self {
        RawSpinLock {
            lock: AtomicBool::new(false),
        }
    }

    /// 尝试获取自旋锁，并返回一个 RAII 保护器。
    ///
    /// 内部原子地获取锁，并在当前 CPU 禁用本地中断。
    pub fn lock(&self) -> RawSpinLockGuard<'_> {
        let guard = IntrGuard::new();

        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            hint::spin_loop();
        }

        RawSpinLockGuard {
            lock: self,
            intr_guard: guard,
        }
    }

    /// 尝试获取自旋锁，如果成功则返回 RAII 保护器，否则返回 None。
    ///
    /// 内部原子地尝试获取锁，并在当前 CPU 禁用本地中断。
    /// 如果获取失败，会立即恢复中断状态（通过 Drop IntrGuard）。
    pub fn try_lock(&self) -> Option<RawSpinLockGuard<'_>> {
        let guard = IntrGuard::new();

        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawSpinLockGuard {
                lock: self,
                intr_guard: guard,
            })
        } else {
            None
        }
    }

    /// 仅释放锁标志。
    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    /// 检查锁是否被占用 (仅用于调试/测试)
    ///
    /// # 返回值
    /// 锁是否被占用
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }
}

/// 自动释放自旋锁和恢复中断状态的 RAII 结构体
pub struct RawSpinLockGuard<'a> {
    lock: &'a RawSpinLock,
    /// 中断保护器，公开以便测试访问
    pub intr_guard: IntrGuard,
}

use core::ops::Drop;

impl Drop for RawSpinLockGuard<'_> {
    /// 退出作用域时自动执行，顺序如下：
    /// 1. 释放自旋锁标志。
    /// 2. IntrGuard 被 Drop，恢复中断状态。
    fn drop(&mut self) {
        self.lock.unlock();
    }
}
