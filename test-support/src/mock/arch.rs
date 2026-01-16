//! 架构相关操作的 Mock 实现

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Mock 架构操作
pub struct MockArchOps {
    pub interrupt_state: AtomicBool,
    pub cpu_id: AtomicUsize,
    pub max_cpus: AtomicUsize,
}

impl MockArchOps {
    pub const fn new() -> Self {
        Self {
            interrupt_state: AtomicBool::new(true),
            cpu_id: AtomicUsize::new(0),
            max_cpus: AtomicUsize::new(1),
        }
    }

    pub unsafe fn read_and_disable_interrupts(&self) -> usize {
        self.interrupt_state.swap(false, Ordering::SeqCst) as usize
    }

    pub unsafe fn restore_interrupts(&self, flags: usize) {
        self.interrupt_state.store(flags != 0, Ordering::SeqCst);
    }

    pub fn sstatus_sie(&self) -> usize {
        0x2 // SIE bit
    }

    pub fn cpu_id(&self) -> usize {
        self.cpu_id.load(Ordering::Relaxed)
    }

    pub fn max_cpu_count(&self) -> usize {
        self.max_cpus.load(Ordering::Relaxed)
    }
}

/// 全局 Mock 实例
pub static MOCK_ARCH_OPS: MockArchOps = MockArchOps::new();
