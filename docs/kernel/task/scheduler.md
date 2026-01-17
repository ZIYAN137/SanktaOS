# 调度器

调度器负责在可运行任务之间做出选择，并在合适的时机触发上下文切换。

本页只描述整体思路与关键路径；细节以源码 rustdoc 为准（见 `os/src/kernel/scheduler/`）。

## 组成

- 调度器入口与公共接口：`os/src/kernel/scheduler/mod.rs`
- 具体调度策略（当前为轮转）：`os/src/kernel/scheduler/rr_scheduler.rs`
- 运行队列：`os/src/kernel/scheduler/task_queue.rs`
- 阻塞/唤醒基础设施：`os/src/kernel/scheduler/wait_queue.rs`

## 典型触发点

- 时钟中断（时间片耗尽触发抢占）
- 任务主动让出（`yield`）
- 任务阻塞/唤醒（等待资源、I/O、同步原语等）

## 上下文切换

调度决策完成后会进入底层切换例程：
- Rust 侧：`os/src/kernel/scheduler/mod.rs`（`schedule()`）
- 架构侧：`os/src/arch/*/kernel/switch.*`（保存/恢复最小上下文）

## 多核注意事项

多核下的唤醒需要避免“同一任务被重复入队/被两个 CPU 同时运行”等问题；相关幂等性处理与 IPI 触发逻辑也在 `os/src/kernel/scheduler/mod.rs` 的唤醒路径中实现。
