//! 简单的任务ID分配器实现

use core::sync::atomic::AtomicU32;

/// 简单的任务ID分配器。
/// 每次调用 `allocate` 返回唯一的任务ID。
/// 任务ID从 2 开始递增（TID 1 保留给 init 进程）。
#[derive(Debug)]
pub struct TidAllocator {
    next_tid: AtomicU32,
}

impl TidAllocator {
    /// 创建一个新的TidAllocator实例。
    pub const fn new() -> Self {
        TidAllocator {
            next_tid: AtomicU32::new(2), // 从2开始，TID 1保留给init进程
        }
    }

    /// 分配一个新的任务ID。
    pub fn allocate(&self) -> u32 {
        self.next_tid
            .fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 顺序分配测试：检查分配值从2开始并依次递增
    #[test_case]
    fn test_tid_allocate_sequence() {
        let alloc = TidAllocator::new();
        assert!(alloc.allocate() == 2);
        assert!(alloc.allocate() == 3);
        assert!(alloc.allocate() == 4);
    }

    // 多引用调用测试：通过多个引用连续分配，确保值唯一且递增
    #[test_case]
    fn test_tid_allocate_multiple_refs() {
        let alloc = TidAllocator::new();
        let a1 = alloc.allocate();
        let a2 = alloc.allocate();
        assert!(a1 == 2);
        assert!(a2 == 3);

        // 通过另一个不可变引用继续分配
        let r = &alloc;
        let a3 = r.allocate();
        assert!(a3 == 4);
    }
}
