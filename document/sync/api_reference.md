# 同步子系统 API 参考

## 概述

本文档补充 `sync` crate 中与“架构相关操作抽象”以及“全局分配器锁集成”相关的 API 说明，便于在 `os` crate 中完成对 `sync` 的对接与注册。

相关源码：

- `crates/sync/src/lib.rs`
- `crates/sync/src/raw_spin_lock_without_guard.rs`

## ArchOps：架构相关操作抽象

`sync` crate 通过 `ArchOps` trait 抽象中断控制与 CPU 信息获取等架构相关能力，并要求在启动早期注册实现。

### trait ArchOps

```rust
pub trait ArchOps: Send + Sync {
    unsafe fn read_and_disable_interrupts(&self) -> usize;
    unsafe fn restore_interrupts(&self, flags: usize);

    fn sstatus_sie(&self) -> usize;
    fn cpu_id(&self) -> usize;
    fn max_cpu_count(&self) -> usize;
}
```

语义约定：

- `read_and_disable_interrupts()`：读取当前中断状态并关闭中断，返回一个可用于恢复的 `flags`。
- `restore_interrupts(flags)`：将中断状态恢复为 `flags` 表示的状态。
- `sstatus_sie()`：返回“中断使能位”常量值（用于位运算判断/设置）。
- `cpu_id()`：返回当前 CPU 的逻辑编号。
- `max_cpu_count()`：返回系统支持的最大 CPU 数量（用于 per-cpu 数据结构初始化等）。

### 注册与获取

```rust
pub unsafe fn register_arch_ops(ops: &'static dyn ArchOps);
```

- 约束：必须在单线程环境下调用且只能调用一次（通常在内核启动早期完成）。
- 失败行为：若未注册而直接使用依赖该接口的功能，会触发 panic（例如 `arch_ops()` 内部检查）。

## RawSpinLockWithoutGuard：不返回 Guard 的原始自旋锁

`RawSpinLockWithoutGuard` 用于与 `lock_api::RawMutex` 集成，主要服务于全局分配器（例如基于 `talc` 的 `Talck`）等场景。

### 关键特性

- **不返回 Guard**：`lock()` 只负责上锁，不返回 RAII guard。
- **内置中断保护**：上锁时会禁用中断并保存 flags；解锁时会恢复中断状态，避免“持有分配器锁时被中断打断，且中断处理路径再次分配内存”导致的死锁。

### 主要 API

```rust
pub struct RawSpinLockWithoutGuard;

impl RawSpinLockWithoutGuard {
    pub const fn new() -> Self;
}

unsafe impl lock_api::RawMutex for RawSpinLockWithoutGuard {
    fn lock(&self);
    fn try_lock(&self) -> bool;
    unsafe fn unlock(&self);
}
```

使用建议：

- 该类型通常不直接使用，而是作为 `lock_api`/分配器适配层的底层 raw mutex。
- `unlock()` 为 `unsafe`：调用者必须保证当前确实持有该锁（由 `lock_api` 的封装类型负责维护此不变量）。

