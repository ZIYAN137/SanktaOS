//! 自旋锁封装
//!
//! 提供对数据的互斥访问的自旋锁结构体。

use core::cell::UnsafeCell;

use crate::raw_spin_lock::{RawSpinLock, RawSpinLockGuard};

/// 提供对数据的互斥访问的自旋锁结构体。
///
/// 内部包含一个 RawSpinLock 和一个 UnsafeCell 用于存储数据。
///
/// # 示例
/// ```ignore
/// let lock = SpinLock::new(0);
/// {
///     let mut guard = lock.lock(); // 获取锁
///     *guard += 1; // 访问和修改数据
/// } // 离开作用域，自动释放锁
/// ```
///
/// # 注意
/// SpinLock 不是可重入的。当持有锁时，尝试再次获取锁将导致死锁。
/// 确保在同一线程中不会嵌套调用 SpinLock::lock()。
/// 此外，SpinLock 通过禁用中断来保护临界区，因此在持有锁时应避免长时间运行的操作。
#[derive(Debug)]
pub struct SpinLock<T> {
    raw_lock: RawSpinLock,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    /// 创建一个新的 SpinLock 实例，初始化内部数据。
    pub const fn new(data: T) -> Self {
        SpinLock {
            raw_lock: RawSpinLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// 获取自旋锁，并返回一个 RAII 保护器，用于访问和修改内部数据。
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let _raw_guard = self.raw_lock.lock();
        SpinLockGuard {
            _raw_guard,
            data: unsafe { &mut *self.data.get() },
        }
    }

    /// 尝试获取自旋锁，如果成功则返回 RAII 保护器，否则返回 None。
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        self.raw_lock.try_lock().map(|_raw_guard| SpinLockGuard {
            _raw_guard,
            data: unsafe { &mut *self.data.get() },
        })
    }

    /// 检查锁是否被占用 (仅用于调试/测试)
    ///
    /// # 返回值
    /// 锁是否被占用
    #[cfg(test)]
    pub fn is_locked(&self) -> bool {
        self.raw_lock.is_locked()
    }
}

/// SpinLock 的 RAII 保护器，提供对锁定数据的访问。
///
/// 当保护器离开作用域时，自动释放锁。
pub struct SpinLockGuard<'a, T> {
    _raw_guard: RawSpinLockGuard<'a>,
    data: &'a mut T,
}

impl<T> core::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> core::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

// Safety: SpinLock 可以在线程间安全共享，
// 因为它通过 RawSpinLock 保证了对数据的互斥访问。
unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}
