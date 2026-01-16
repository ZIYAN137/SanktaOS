//! 同步原语
//!
//! 向其它内核模块提供基本的锁和同步原语
//! 包括自旋锁、睡眠锁、中断保护等

mod mutex;
mod per_cpu;

pub use mutex::*;
pub use per_cpu::PerCpu;

// 从 sync crate re-export
pub use sync::{
    IntrGuard, PreemptGuard, RawSpinLock, RawSpinLockGuard, RawSpinLockWithoutGuard, RwLock,
    RwLockReadGuard, RwLockWriteGuard, SpinLock, SpinLockGuard, TicketLock, TicketLockGuard,
    preempt_disable, preempt_disabled, preempt_enable,
};
