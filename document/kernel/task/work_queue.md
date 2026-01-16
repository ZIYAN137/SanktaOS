# 工作队列（Work Queue）

## 概述

工作队列（Work Queue）是一种延迟执行机制，用于将耗时或非紧急的任务推迟到合适的时机执行，避免在关键路径上阻塞。工作队列由专门的工作线程（worker thread）负责处理，这些线程在后台异步执行提交的工作项。

**典型应用场景**：
- 任务资源清理：任务终止时，将资源清理操作放入工作队列
- 延迟 I/O 操作：将非紧急的磁盘写入操作延迟执行
- 异步通知：将通知操作推迟到合适的时机

**代码位置**：`os/src/kernel/task/work_queue.rs`

## 核心概念

### 工作队列模型

```
提交者线程                工作队列                工作线程
    |                       |                       |
    |-- schedule_work() -->|                       |
    |                       |-- 加入队列 -->        |
    |                       |                       |
    |                       |                  <-- pop_front()
    |                       |                       |
    |                       |                   执行工作项
    |                       |                       |
    |                       |                   继续循环
```

### 工作线程生命周期

1. **初始化**：工作线程启动后，将自己注册到工作队列
2. **工作循环**：
   - 从队列中取出工作项并执行
   - 如果队列为空，进入休眠状态
   - 被唤醒后继续处理工作项
3. **永不退出**：工作线程持续运行，直到系统关闭

## 核心数据结构

### WorkItem

工作项，表示一个待执行的任务。

```rust
pub struct WorkItem {
    pub task: fn(),
}
```

**字段**：
- `task` - 函数指针，指向要执行的任务函数

**限制**：
- 当前只支持无参数、无返回值的函数
- 不支持闭包或带状态的任务

---

### WorkQueue

工作队列，管理工作项和工作线程。

```rust
pub struct WorkQueue {
    sleeping: usize,
    worker: Vec<SharedTask>,
    work_queue: VecDeque<WorkItem>,
}
```

**字段**：
- `sleeping` - 当前处于休眠状态的工作线程数量
- `worker` - 工作线程列表
- `work_queue` - 待处理的工作项队列（FIFO）

---

### GLOBAL_WORK_QUEUE

全局工作队列实例。

```rust
pub static ref GLOBAL_WORK_QUEUE: SpinLock<WorkQueue>
```

**特点**：
- 使用 `lazy_static!` 宏实现延迟初始化
- 使用 `SpinLock` 保护，确保并发安全
- 全局唯一，所有工作项都提交到这个队列

## API 参考

### WorkItem::new()

```rust
pub fn new(task: fn()) -> Self
```

创建一个新的工作项。

**参数**：
- `task` - 要执行的任务函数（无参数、无返回值）

**返回值**：新创建的工作项

**示例**：
```rust
fn cleanup_task() {
    println!("Cleaning up resources...");
}

let work = WorkItem::new(cleanup_task);
```

---

### WorkQueue::new()

```rust
pub fn new() -> Self
```

创建一个新的工作队列实例。

**返回值**：新创建的工作队列

**注意**：通常不需要手动创建，应使用全局的 `GLOBAL_WORK_QUEUE` 实例。

---

### WorkQueue::schedule_work()

```rust
pub fn schedule_work(&mut self, work: WorkItem)
```

将工作项加入工作队列，并唤醒一个休眠的工作线程。

**参数**：
- `work` - 要提交的工作项

**行为**：
1. 将工作项加入队列尾部（FIFO）
2. 如果有休眠的工作线程，唤醒其中一个
3. 如果所有工作线程都在忙碌，工作项会在队列中等待

**示例**：
```rust
fn my_task() {
    println!("Executing delayed task");
}

let work = WorkItem::new(my_task);
GLOBAL_WORK_QUEUE.lock().schedule_work(work);
```

---

### WorkQueue::add_worker()

```rust
pub fn add_worker(&mut self, task: SharedTask)
```

添加工作线程到工作队列。

**参数**：
- `task` - 工作线程的任务结构

**注意**：此方法通常由工作线程自己调用，在启动时注册自己。

---

### kworker()

```rust
pub fn kworker()
```

工作线程主函数，永不返回。

**行为**：
1. 将当前线程注册为工作线程
2. 进入无限循环：
   - 从队列中取出工作项并执行
   - 如果队列为空，进入休眠状态
   - 被唤醒后继续处理

**示例**：
```rust
// 创建工作线程
kernel::spawn_kernel_thread(kworker, "kworker");
```

## 使用场景

### 1. 延迟资源清理

```rust
// 任务终止时的清理操作
fn cleanup_terminated_task() {
    // 释放文件描述符
    // 释放内存空间
    // 通知父进程
    println!("Task cleanup completed");
}

// 在任务终止时提交清理工作
pub fn on_task_exit() {
    let work = WorkItem::new(cleanup_terminated_task);
    GLOBAL_WORK_QUEUE.lock().schedule_work(work);
}
```

### 2. 异步日志写入

```rust
// 将日志缓冲区刷新到磁盘
fn flush_log_buffer() {
    // 将内存中的日志写入磁盘
    println!("Flushing log buffer to disk");
}

// 定期提交日志刷新任务
pub fn schedule_log_flush() {
    let work = WorkItem::new(flush_log_buffer);
    GLOBAL_WORK_QUEUE.lock().schedule_work(work);
}
```

### 3. 延迟通知

```rust
// 通知其他子系统
fn notify_subsystem() {
    // 发送通知消息
    println!("Notifying subsystem");
}

// 在某个事件发生后延迟通知
pub fn on_event() {
    let work = WorkItem::new(notify_subsystem);
    GLOBAL_WORK_QUEUE.lock().schedule_work(work);
}
```

## 实现细节

### 工作项执行

- **FIFO 顺序**：工作项按照提交顺序执行（先进先出）
- **同步执行**：工作线程同步执行工作项，一次只执行一个
- **无超时**：工作项执行没有超时限制，可能长时间阻塞工作线程

### 工作线程管理

- **动态注册**：工作线程在启动时自己注册到工作队列
- **休眠机制**：队列为空时，工作线程进入 `Interruptible` 状态休眠
- **唤醒策略**：有新工作项时，只唤醒一个休眠的工作线程

### 并发控制

- **全局锁**：`GLOBAL_WORK_QUEUE` 使用 `SpinLock` 保护
- **锁粒度**：每次操作都需要获取全局锁，可能成为性能瓶颈
- **死锁风险**：工作项函数中不应再次获取 `GLOBAL_WORK_QUEUE` 的锁

### 内存管理

- **VecDeque**：使用 `VecDeque` 存储工作项，支持高效的队首/队尾操作
- **无界队列**：工作队列没有大小限制，可能导致内存耗尽
- **无清理机制**：工作线程列表不会自动清理已终止的线程

## 与 Linux 工作队列的对比

| 特性 | SanktaOS | Linux |
|------|----------|-------|
| 工作项类型 | 函数指针 | 结构体 + 回调函数 |
| 队列类型 | 单一全局队列 | 多种队列（系统、per-CPU、自定义）|
| 优先级 | 不支持 | 支持（高优先级队列）|
| 延迟执行 | 不支持 | 支持（delayed_work）|
| 取消操作 | 不支持 | 支持（cancel_work）|
| 工作线程数 | 手动创建 | 动态调整 |
| 并发控制 | 全局锁 | per-CPU 队列 + 细粒度锁 |

## 已知限制

1. **功能简单**：只支持基本的工作项提交和执行，缺少高级功能
2. **无优先级**：所有工作项按 FIFO 顺序执行，无法区分优先级
3. **无延迟执行**：不支持定时执行或延迟执行
4. **无取消机制**：一旦提交，工作项无法取消
5. **无界队列**：工作队列没有大小限制，可能导致内存耗尽
6. **全局锁瓶颈**：所有操作都需要获取全局锁，高并发场景下性能较差
7. **工作项限制**：只支持无参数、无返回值的函数，不支持闭包或带状态的任务
8. **无错误处理**：工作项执行失败时没有错误处理机制

## 未来改进方向

1. **支持闭包和状态**：
   ```rust
   pub struct WorkItem {
       task: Box<dyn FnOnce() + Send>,
   }
   ```

2. **添加优先级支持**：
   - 高优先级队列和低优先级队列
   - 优先处理高优先级工作项

3. **实现延迟执行**：
   ```rust
   pub struct DelayedWork {
       work: WorkItem,
       delay: Duration,
   }
   ```

4. **添加取消机制**：
   ```rust
   pub fn cancel_work(&mut self, work_id: usize) -> bool
   ```

5. **实现有界队列**：
   - 设置队列大小限制
   - 队列满时拒绝新工作项或阻塞提交者

6. **优化并发性能**：
   - 使用 per-CPU 工作队列
   - 减少锁竞争

7. **动态工作线程管理**：
   - 根据负载自动创建/销毁工作线程
   - 设置最小/最大工作线程数

8. **添加错误处理**：
   - 捕获工作项执行中的 panic
   - 提供错误回调机制

9. **统计和监控**：
   - 记录工作项执行时间
   - 统计队列长度和工作线程利用率

## 使用建议

1. **避免长时间阻塞**：工作项应该快速完成，避免长时间阻塞工作线程
2. **避免死锁**：工作项函数中不应再次获取 `GLOBAL_WORK_QUEUE` 的锁
3. **控制提交速率**：避免短时间内提交大量工作项，导致队列积压
4. **创建足够的工作线程**：根据系统负载创建适当数量的工作线程
5. **考虑使用专用队列**：对于特定类型的任务，可以考虑创建专用的工作队列

## 参考

- 源代码：`os/src/kernel/task/work_queue.rs`
- 相关模块：
  - 任务管理：`kernel::task`
  - 自旋锁：`sync::SpinLock`
  - 任务状态：`kernel::TaskState`
- Linux 文档：
  - `Documentation/core-api/workqueue.rst`
  - `include/linux/workqueue.h`
