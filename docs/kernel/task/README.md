# 任务管理概述

任务管理子系统负责创建/销毁任务（Task）、维护任务状态，并与调度器、同步原语、内存管理、文件描述符等子系统协作完成“进程/线程运行时”的核心能力。

说明：本目录仅保留概览类文档；实现细节与 API 语义以对应源码文件的 rustdoc 注释为准。

## 关键概念

- **Task**：内核中用于表示执行实体的基本抽象；在多数路径上不严格区分“进程/线程”，而是通过资源共享关系（地址空间、文件表等）体现差异。
- **生命周期**：创建 → 就绪/运行 → 阻塞/唤醒 → 退出与资源回收。

## 源码导览（以源码 rustdoc 为准）

- 任务模块入口：`os/src/kernel/task/mod.rs`
- 任务结构与资源聚合：`os/src/kernel/task/task_struct.rs`
- 进程/线程相关逻辑：`os/src/kernel/task/process.rs`
- 任务管理器：`os/src/kernel/task/task_manager.rs`
- TID 分配：`os/src/kernel/task/tid_allocator.rs`
- `exec` 装载：`os/src/kernel/task/exec_loader.rs`
- Futex：`os/src/kernel/task/futex.rs`
- 工作队列：`os/src/kernel/task/work_queue.rs`
- 能力（capability）：`os/src/kernel/task/cap.rs`
- 凭证（credential）：`os/src/kernel/task/cred.rs`

与调度/阻塞直接相关的实现位于调度器模块：
- 调度器入口：`os/src/kernel/scheduler/mod.rs`
- 轮转调度：`os/src/kernel/scheduler/rr_scheduler.rs`
- 等待队列：`os/src/kernel/scheduler/wait_queue.rs`

## 进一步阅读

- [调度器](scheduler.md)
