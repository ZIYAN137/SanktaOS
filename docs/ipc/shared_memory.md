# 共享内存

## 概述

共享内存（Shared Memory）是一种高效的进程间通信（IPC）机制，允许多个进程访问同一块物理内存区域。SanktaOS 提供了简单的共享内存管理功能，支持创建、映射和移除共享内存段。

**代码位置**：`os/src/ipc/shared_memory.rs`

## 核心数据结构

### SharedMemoryTable

共享内存表，用于管理进程的所有共享内存段。

```rust
pub struct SharedMemoryTable {
    memory: Vec<Arc<SharedMemory>>,
}
```

**职责**：
- 维护进程持有的共享内存段列表
- 提供创建、移除共享段的接口
- 使用 `Arc` 实现共享内存的引用计数

### SharedMemory

共享内存段，持有一组连续的物理页。

```rust
pub struct SharedMemory {
    frames: Vec<FrameTracker>,
    len: usize,
}
```

**职责**：
- 持有物理页帧（通过 `FrameTracker` 实现 RAII）
- 提供映射到用户空间的功能
- 自动管理物理内存的生命周期

## API 参考

### SharedMemoryTable API

#### new()

```rust
pub fn new() -> Self
```

创建一个空的共享内存表。

**返回值**：新创建的共享内存表

**示例**：
```rust
let shm_table = SharedMemoryTable::new();
```

---

#### create()

```rust
pub fn create(&mut self, pages: usize) -> Arc<SharedMemory>
```

创建一个新的共享内存段并登记到表中。

**参数**：
- `pages` - 共享段的页数（每页 4KB）

**返回值**：指向新创建共享段的 `Arc` 句柄

**示例**：
```rust
let mut shm_table = SharedMemoryTable::new();
// 创建 10 页（40KB）的共享内存
let shm = shm_table.create(10);
```

**注意**：
- 物理页在创建时立即分配
- 如果物理内存不足，会触发 panic
- 返回的 `Arc` 可以在多个进程间共享

---

#### remove()

```rust
pub fn remove(&mut self, shm: &Arc<SharedMemory>) -> bool
```

从表中移除指定的共享内存段。

**参数**：
- `shm` - 要移除的共享段的引用

**返回值**：
- `true` - 成功移除
- `false` - 共享段不在表中

**示例**：
```rust
let shm = shm_table.create(10);
let removed = shm_table.remove(&shm);
assert!(removed);
```

**注意**：
- 移除操作只是从表中删除引用
- 如果其他地方仍持有 `Arc`，物理内存不会立即释放
- 当前实现不会自动取消用户空间的映射（见代码注释 XXX）

---

#### len() / is_empty()

```rust
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```

查询表中共享段的数量。

**示例**：
```rust
assert_eq!(shm_table.len(), 0);
assert!(shm_table.is_empty());
```

---

### SharedMemory API

#### new()

```rust
pub fn new(pages: usize) -> Self
```

分配指定数量的物理页作为共享内存段。

**参数**：
- `pages` - 要分配的页数

**返回值**：新创建的共享内存段

**Panic**：如果物理内存不足

**示例**：
```rust
let shm = SharedMemory::new(10);  // 分配 10 页（40KB）
```

---

#### len() / is_empty()

```rust
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```

查询共享段的字节数。

**示例**：
```rust
let shm = SharedMemory::new(10);
assert_eq!(shm.len(), 10 * 4096);  // 40KB
```

---

#### map_to_user()

```rust
pub fn map_to_user(self) -> Result<usize, PagingError>
```

将共享内存段映射到当前进程的用户空间。

**返回值**：
- `Ok(usize)` - 成功，返回映射的虚拟地址
- `Err(PagingError)` - 失败，返回错误信息

**权限**：映射的页具有以下权限：
- `READABLE` - 可读
- `WRITEABLE` - 可写
- `USER_ACCESSIBLE` - 用户态可访问
- `VALID` - 有效

**示例**：
```rust
let shm = SharedMemory::new(10);
match shm.map_to_user() {
    Ok(addr) => println!("Mapped at: 0x{:x}", addr),
    Err(e) => eprintln!("Mapping failed: {:?}", e),
}
```

**注意**：
- 此方法会消耗 `self`（获取所有权）
- 映射地址由内存管理器自动选择（传入 0 表示自动选择）
- 映射后，进程可以通过返回的虚拟地址访问共享内存

## 使用场景

### 1. 进程间数据共享

多个进程可以通过共享内存高效地交换大量数据，避免数据拷贝的开销。

```rust
// 进程 A：创建并映射共享内存
let mut shm_table = SharedMemoryTable::new();
let shm = shm_table.create(10);
let addr = shm.map_to_user().expect("Failed to map");

// 写入数据
unsafe {
    let ptr = addr as *mut u8;
    ptr.write(42);
}

// 进程 B：通过某种方式获取 Arc<SharedMemory>
// 然后映射到自己的地址空间
let addr_b = shm.map_to_user().expect("Failed to map");

// 读取数据
unsafe {
    let ptr = addr_b as *const u8;
    let value = ptr.read();
    assert_eq!(value, 42);
}
```

### 2. 父子进程通信

父进程创建共享内存后，通过 `fork()` 传递给子进程。

```rust
// 父进程
let shm = shm_table.create(10);
let addr = shm.map_to_user().expect("Failed to map");

// fork 后，子进程继承共享内存的 Arc
// 子进程可以直接访问相同的物理内存
```

## 实现细节

### 内存管理

- **物理页分配**：使用 `alloc_frames()` 一次性分配所有物理页
- **RAII 机制**：通过 `FrameTracker` 自动管理物理页的生命周期
- **引用计数**：使用 `Arc` 实现共享内存的引用计数，当所有引用释放时自动回收物理内存

### 映射机制

- **自动地址选择**：调用 `space.mmap(0, len, flags)` 时，传入 0 表示由内存管理器自动选择合适的虚拟地址
- **权限设置**：映射的页具有读写权限，且用户态可访问
- **当前进程**：`map_to_user()` 只能映射到当前进程的地址空间

### 已知限制

1. **移除操作不完整**：`SharedMemoryTable::remove()` 只是从表中删除引用，不会自动取消用户空间的映射（见代码注释 XXX）
2. **缺少跨进程共享机制**：当前实现没有提供在不同进程间传递 `Arc<SharedMemory>` 的标准方法
3. **缺少同步机制**：共享内存本身不提供同步原语，需要配合其他 IPC 机制（如信号量、互斥锁）使用

## 与 Linux 的对比

| 特性 | SanktaOS | Linux |
|------|----------|-------|
| 创建方式 | `SharedMemoryTable::create()` | `shmget()` |
| 映射方式 | `map_to_user()` | `shmat()` |
| 移除方式 | `remove()` | `shmdt()` + `shmctl()` |
| 标识符 | `Arc<SharedMemory>` | IPC key / shmid |
| 权限控制 | 固定为 RW | 可配置（0666 等）|
| 同步机制 | 需外部实现 | 可配合信号量 |

## 未来改进方向

1. **完善移除操作**：在 `remove()` 时自动取消用户空间的映射
2. **添加权限控制**：允许指定共享内存的访问权限
3. **实现 IPC 标识符**：提供类似 Linux shmid 的机制，方便跨进程共享
4. **添加同步原语**：提供内置的互斥锁或信号量支持
5. **支持命名共享内存**：允许通过名称在不同进程间查找共享内存段

## 参考

- 源代码：`os/src/ipc/shared_memory.rs`
- 相关模块：
  - 物理帧分配器：`mm::frame_allocator`
  - 内存空间管理：`mm::memory_space`
  - 页表管理：`mm::page_table`
