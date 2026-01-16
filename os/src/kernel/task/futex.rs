//! Futex 相关功能
//!
//! 该模块提供一个全局 Futex 管理器，用于维护“地址 -> 等待队列”的映射。
//!
//! ## 地址选择
//!
//! 内核在处理 futex 系统调用时通常会先将用户态地址转换为可用作 key 的地址：
//!
//! - 当前实现将用户虚拟地址翻译为物理地址（`paddr`），并以 `paddr` 作为 key，
//!   以便同一物理页上的 futex 能共享等待队列。
//!
//! ## 目前支持范围
//!
//! Futex 的 WAIT/WAKE 等具体语义由系统调用实现（见 `os/src/kernel/syscall/task.rs`），
//! 本模块仅提供等待队列的管理。

use hashbrown::HashMap;

use crate::kernel::WaitQueue;
use crate::sync::SpinLock;

lazy_static::lazy_static! {
    /// 全局 Futex 管理器实例
    pub static ref FUTEX_MANAGER: SpinLock<FutexManager> = SpinLock::new(FutexManager::new());
}

/// Futex 管理器，负责管理所有 Futex 等待队列。
pub struct FutexManager {
    /// key（通常为转换后的地址）到等待队列的映射。
    futexes: HashMap<usize, WaitQueue>,
}

impl FutexManager {
    /// 创建一个新的 Futex 管理器实例
    pub fn new() -> Self {
        Self {
            futexes: HashMap::new(),
        }
    }

    /// 根据 key 获取对应的 Futex 等待队列。
    ///
    /// 若等待队列不存在，则会新建并返回。
    pub fn get_wait_queue(&mut self, uaddr: usize) -> &mut WaitQueue {
        self.futexes.entry(uaddr).or_insert_with(WaitQueue::new)
    }
}
