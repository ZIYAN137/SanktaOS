# 日志系统概述

Log 子系统用于内核日志记录，提供类似 Linux `printk` 的 `pr_*` 宏，并支持：
- 按级别过滤（全局缓存级别与控制台输出级别分离）
- 环形缓冲区缓存（供 `syslog` 系统调用读取）
- 早期启动可用（由 OS 层在启动流程中完成注册/初始化）

## 源码导览（以源码 rustdoc 为准）

- OS 层包装与初始化：`os/src/log/mod.rs`（`init()` + `pr_*` 宏）
- 日志核心实现：`crates/klog/src/`（环形缓冲区、条目格式化、级别定义等）
- `syslog` 系统调用：`os/src/kernel/syscall/sys.rs`（`syslog`）
- `syslog` action 定义：`crates/uapi/src/log.rs`（`SyslogAction`）

## 快速使用（内核侧）

```rust
pr_info!("kernel init ok");
pr_warn!("low memory: {}", free_pages);
pr_err!("mount failed: {}", errno);
```

说明：日志系统需要在早期启动时调用 `crate::log::init()` 注册上下文提供者与输出实现；
之后即可使用 `pr_*` 宏记录日志。
