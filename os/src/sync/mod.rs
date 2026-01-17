//! 同步原语
//!
//! 向其它内核模块提供基本的锁和同步原语
//! 包括自旋锁、读写锁、票号锁、中断保护、抢占控制、Per-CPU 等
//!
//! # 锁顺序（Lock Ordering）与死锁预防
//!
//! 本项目的同步原语本身并不会“自动防死锁”。一旦在同一条执行路径上需要嵌套获取多把锁，
//! 就必须遵循一致的锁顺序，避免循环等待。
//!
//! 经验上，建议遵循“从更全局/更粗粒度到更局部/更细粒度”的顺序；与此同时，为了可落地，
//! 这里给出一份**项目常见锁**的参考层级表（从高到低）：
//!
//! | 层级（高→低） | 锁（示例） | 备注 |
//! |---|---|---|
//! | 1 | `TASK_MANAGER`（全局 `SpinLock`） | 全局任务表、TID 分配等 |
//! | 2 | `WaitQueue` 相关锁（外层 `SpinLock<WaitQueue>` 与内部 `WaitQueue.lock: RawSpinLock`） | sleep/wake 可能触达调度器与任务状态 |
//! | 3 | 调度器锁（`current_scheduler()` / `scheduler_of()` 的 `SpinLock`） | 调度器是 per-CPU 的，不是单一全局锁 |
//! | 4 | 单个任务实例锁（`task.lock()` / `SpinLock<TaskStruct>`） | 任务状态与上下文等 |
//! | 5 | 任务内部字段锁（例如 `children`、`wait_child` 等） | 更细粒度共享字段，尽量最后获取 |
//!
//! 注意：
//! - CPU 本地状态访问依赖 `PreemptGuard`（用于防止任务迁移），它不是“锁”，但如果需要同时使用
//!   `PreemptGuard` 与多把锁，通常建议先进入 `PreemptGuard`，再按上述顺序获取其它锁。
//! - 若不确定某个调用是否会隐式获取其它锁（例如 wait/schedule/wake 路径），优先把“取引用/取快照”
//!   与“持锁操作”拆开，尽量缩短持锁时间。

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
