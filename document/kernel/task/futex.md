# Futex（Fast Userspace Mutex）

## 概述

Futex（Fast Userspace Mutex，快速用户空间互斥锁）是一种高效的同步机制，允许用户态程序实现各种同步原语（如互斥锁、条件变量、信号量等），只在必要时才陷入内核。

SanktaOS 提供了基础的 Futex 支持，通过维护用户空间地址到等待队列的映射，实现进程/线程的阻塞和唤醒功能。

**代码位置**：`os/src/kernel/task/futex.rs`

## 核心概念

### Futex 工作原理

Futex 的核心思想是：
1. **用户态快速路径**：大多数情况下，锁操作在用户态完成，无需系统调用
2. **内核态慢速路径**：只有在发生竞争时，才通过系统调用进入内核进行阻塞/唤醒

**典型流程**：
```
用户态尝试获取锁（原子操作）
  ├─ 成功 → 继续执行（快速路径）
  └─ 失败 → futex_wait() 系统调用，进入内核阻塞（慢速路径）

用户态释放锁
  ├─ 无等待者 → 直接返回（快速路径）
  └─ 有等待者 → futex_wake() 系统调用，唤醒等待线程（慢速路径）
```

### 用户空间地址作为标识

Futex 使用用户空间的内存地址作为唯一标识：
- 不同的地址对应不同的 Futex 对象
- 多个线程可以在同一个地址上等待
- 内核维护地址到等待队列的映射

## 核心数据结构

### FutexManager

Futex 管理器，负责管理所有的 Futex 对象。

```rust
pub struct FutexManager {
    futexes: HashMap<usize, WaitQueue>,
}
```

**职责**：
- 维护用户空间地址到等待队列的映射
- 提供获取等待队列的接口
- 自动创建不存在的等待队列

### FUTEX_MANAGER

全局 Futex 管理器实例。

```rust
pub static ref FUTEX_MANAGER: SpinLock<FutexManager>
```

**特点**：
- 使用 `lazy_static!` 宏实现延迟初始化
- 使用 `SpinLock` 保护，确保并发安全
- 全局唯一，所有 Futex 操作都通过它进行

## API 参考

### FutexManager::new()

```rust
pub fn new() -> Self
```

创建一个新的 Futex 管理器实例。

**返回值**：新创建的 Futex 管理器

**示例**：
```rust
let manager = FutexManager::new();
```

**注意**：通常不需要手动创建，应使用全局的 `FUTEX_MANAGER` 实例。

---

### FutexManager::get_wait_queue()

```rust
pub fn get_wait_queue(&mut self, uaddr: usize) -> &mut WaitQueue
```

根据用户空间地址获取对应的 Futex 等待队列。

**参数**：
- `uaddr` - 用户空间的内存地址（作为 Futex 的唯一标识）

**返回值**：指向等待队列的可变引用

**行为**：
- 如果该地址的等待队列已存在，直接返回
- 如果不存在，自动创建一个新的等待队列并返回

**示例**：
```rust
let mut manager = FUTEX_MANAGER.lock();
let wait_queue = manager.get_wait_queue(0x1000);
// 现在可以对 wait_queue 进行操作（阻塞、唤醒等）
```

## 使用场景

### 1. 实现用户态互斥锁

```rust
// 用户态代码（伪代码）
struct Mutex {
    futex: AtomicU32,  // 0 = 未锁定, 1 = 已锁定
}

impl Mutex {
    fn lock(&self) {
        // 快速路径：尝试原子地将 0 改为 1
        if self.futex.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            return;  // 成功获取锁
        }

        // 慢速路径：进入内核等待
        loop {
            // futex_wait 系统调用
            syscall(SYS_FUTEX, &self.futex, FUTEX_WAIT, 1);

            // 被唤醒后再次尝试获取锁
            if self.futex.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                return;
            }
        }
    }

    fn unlock(&self) {
        // 释放锁
        self.futex.store(0, Ordering::Release);

        // 唤醒一个等待者
        syscall(SYS_FUTEX, &self.futex, FUTEX_WAKE, 1);
    }
}
```

### 2. 实现条件变量

```rust
// 用户态代码（伪代码）
struct CondVar {
    futex: AtomicU32,
}

impl CondVar {
    fn wait(&self, mutex: &Mutex) {
        // 1. 释放互斥锁
        mutex.unlock();

        // 2. 在条件变量上等待
        syscall(SYS_FUTEX, &self.futex, FUTEX_WAIT, self.futex.load(Ordering::Relaxed));

        // 3. 被唤醒后重新获取互斥锁
        mutex.lock();
    }

    fn notify_one(&self) {
        // 唤醒一个等待者
        self.futex.fetch_add(1, Ordering::Release);
        syscall(SYS_FUTEX, &self.futex, FUTEX_WAKE, 1);
    }

    fn notify_all(&self) {
        // 唤醒所有等待者
        self.futex.fetch_add(1, Ordering::Release);
        syscall(SYS_FUTEX, &self.futex, FUTEX_WAKE, i32::MAX);
    }
}
```

### 3. 内核态使用

```rust
// 内核态代码
use crate::kernel::task::futex::FUTEX_MANAGER;

// FUTEX_WAIT 系统调用实现
pub fn sys_futex_wait(uaddr: usize, expected: u32) -> isize {
    // 1. 检查用户空间地址的值
    let current_value = unsafe { *(uaddr as *const u32) };
    if current_value != expected {
        return -EAGAIN;  // 值已改变，不需要等待
    }

    // 2. 获取等待队列并阻塞当前任务
    let mut manager = FUTEX_MANAGER.lock();
    let wait_queue = manager.get_wait_queue(uaddr);
    wait_queue.sleep();  // 阻塞当前任务

    0
}

// FUTEX_WAKE 系统调用实现
pub fn sys_futex_wake(uaddr: usize, count: i32) -> isize {
    // 获取等待队列并唤醒指定数量的任务
    let mut manager = FUTEX_MANAGER.lock();
    let wait_queue = manager.get_wait_queue(uaddr);
    let woken = wait_queue.wakeup_n(count as usize);

    woken as isize
}
```

## 实现细节

### 地址映射

- **HashMap 存储**：使用 `HashMap<usize, WaitQueue>` 存储地址到等待队列的映射
- **延迟创建**：等待队列在首次访问时才创建（`or_insert_with`）
- **自动清理**：当前实现不会自动清理空的等待队列（可能导致内存泄漏）

### 并发控制

- **全局锁**：`FUTEX_MANAGER` 使用 `SpinLock` 保护
- **性能影响**：所有 Futex 操作都需要获取全局锁，可能成为性能瓶颈
- **改进方向**：可以考虑使用更细粒度的锁（如每个地址一个锁）

### 等待队列

- **WaitQueue**：内核提供的等待队列机制
- **阻塞操作**：`wait_queue.sleep()` 将当前任务加入队列并阻塞
- **唤醒操作**：`wait_queue.wakeup_n(count)` 唤醒指定数量的任务

## 与 Linux Futex 的对比

| 特性 | SanktaOS | Linux |
|------|----------|-------|
| 基本操作 | WAIT / WAKE | WAIT / WAKE / REQUEUE / CMP_REQUEUE 等 |
| 超时支持 | 未实现 | 支持（FUTEX_WAIT_TIMEOUT）|
| 私有/共享 | 未区分 | 支持（FUTEX_PRIVATE_FLAG）|
| 位掩码 | 未实现 | 支持（FUTEX_BITSET）|
| 优先级继承 | 未实现 | 支持（FUTEX_LOCK_PI）|
| 地址清理 | 未实现 | 自动清理空队列 |

## 已知限制

1. **功能不完整**：只实现了基本的 WAIT/WAKE 操作，缺少 REQUEUE、CMP_REQUEUE 等高级功能
2. **缺少超时支持**：无法设置等待超时时间
3. **内存泄漏风险**：空的等待队列不会自动清理，长时间运行可能导致内存泄漏
4. **性能瓶颈**：全局锁可能成为高并发场景下的性能瓶颈
5. **缺少私有/共享区分**：所有 Futex 都是全局共享的，无法区分进程私有和跨进程共享

## 未来改进方向

1. **实现完整的 Futex 操作**：
   - `FUTEX_REQUEUE` - 将等待者从一个 Futex 移动到另一个
   - `FUTEX_CMP_REQUEUE` - 条件性地重新排队
   - `FUTEX_WAKE_OP` - 唤醒并执行原子操作

2. **添加超时支持**：
   - `FUTEX_WAIT_TIMEOUT` - 带超时的等待
   - 使用定时器机制实现

3. **优化性能**：
   - 使用细粒度锁减少锁竞争
   - 考虑使用无锁数据结构

4. **自动清理**：
   - 定期清理空的等待队列
   - 或在等待队列为空时立即删除

5. **支持私有 Futex**：
   - 区分进程私有和跨进程共享的 Futex
   - 私有 Futex 可以使用更高效的实现

6. **优先级继承**：
   - 实现 `FUTEX_LOCK_PI` 支持
   - 防止优先级反转问题

## 参考

- 源代码：`os/src/kernel/task/futex.rs`
- 相关模块：
  - 等待队列：`kernel::WaitQueue`
  - 自旋锁：`sync::SpinLock`
- Linux 文档：
  - `man 2 futex`
  - `man 7 futex`
- 经典论文：
  - "Fuss, Futexes and Furwocks: Fast Userlevel Locking in Linux" (2002)
