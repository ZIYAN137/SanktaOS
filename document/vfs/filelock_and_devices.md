# 文件锁与设备管理

## 概述

本文档介绍 VFS 的文件锁机制（`fcntl`/POSIX advisory locks）以及设备管理相关接口。

相关源码：

- 文件锁：`crates/vfs/src/file_lock.rs`、`os/src/kernel/syscall/fcntl.rs`
- 设备接口：`crates/vfs/src/ops.rs`
- 设备号与映射：`crates/vfs/src/dev.rs`、`crates/vfs/src/devno.rs`

## 文件锁（POSIX advisory locks）

VFS 当前实现的是 POSIX advisory locks（建议性锁）：

- 锁不会自动强制约束所有读写路径，是否遵守取决于调用方是否通过 `fcntl` 设置/检查锁。
- 锁以“文件标识 + 字节区间 + PID”为维度管理。

### 锁类型

使用 `uapi::fcntl::LockType`：

- `Read`：读锁（共享锁）
- `Write`：写锁（独占锁）
- `Unlock`：解锁

锁冲突语义（同一文件区间）：

| 已持有 \\ 请求 | 读锁 | 写锁 |
|---|---|---|
| 无锁 | 允许 | 允许 |
| 读锁 | 允许（共享） | 冲突 |
| 写锁 | 冲突 | 冲突 |

补充规则：

- 同一进程（相同 PID）的锁不视为冲突（可以覆盖/升级/降级）。

### FileLockManager：完整 API

全局文件锁管理器位于 `crates/vfs/src/file_lock.rs`：

```rust
pub struct FileLockManager;

pub fn file_lock_manager() -> &'static FileLockManager;
```

核心方法：

```rust
pub fn test_lock(
    &self,
    dev: u64,
    ino: u64,
    start: usize,
    len: usize,
    flock: &mut Flock,
    pid: i32,
) -> Result<(), FsError>;

pub fn set_lock(
    &self,
    dev: u64,
    ino: u64,
    start: usize,
    len: usize,
    lock_type: LockType,
    pid: i32,
    blocking: bool,
) -> Result<(), FsError>;

pub fn release_all_locks(&self, pid: i32);
```

参数说明：

- `dev`/`ino`：用于标识文件（设备号 + inode 号）。
- `start`/`len`：文件区间的**绝对偏移**范围。
- `pid`：持有/请求锁的进程 PID。
- `flock`：`F_GETLK` 的输入与输出载体。

### 与系统调用的关系

`os/src/kernel/syscall/fcntl.rs` 会在处理 `F_GETLK` / `F_SETLK` / `F_SETLKW` 时：

1. 从文件对象读取 `offset` 与 `metadata.size`
2. 调用 `Flock::to_absolute_range(file_offset, file_size)` 将 `flock` 转换为绝对区间 `(start, len)`
3. 调用 `file_lock_manager().test_lock(...)` 或 `file_lock_manager().set_lock(...)`

注意：当前系统调用侧对 `dev` 仍有 TODO（暂时传入 `dev = 0`），依赖 inode 号区分文件。后续若引入多文件系统/多设备挂载，需要为每个文件系统分配稳定的设备号。

### 行为细节与限制

- `test_lock`：
  - 输入：`flock.l_type` 必须为读锁或写锁
  - 输出：若存在冲突锁则回填 `flock`（类型、范围、PID 等）；否则将 `flock.l_type` 设置为 `Unlock`
- `set_lock`：
  - 冲突处理：遇到冲突立即返回 `FsError::WouldBlock`
  - `blocking` 参数当前未生效（`F_SETLKW` 尚未实现阻塞等待）
- 进程退出清理：调用 `release_all_locks(pid)` 移除该 PID 的所有锁
- 未实现/待完善：
  - `F_SETLKW` 阻塞等待（需要等待队列、信号中断等机制）
  - 死锁检测
  - 锁区间合并/拆分（避免锁表膨胀）

## 设备管理接口

VFS 将设备相关能力抽象为运行时接口，由 `os` crate 提供实现并在启动阶段注册。

- 运行时接口说明见：[`vfs/ops.md`](./ops.md)
- 设备号与映射规则见：[`vfs/devno.md`](./devno.md)

### 字符设备与块设备

VFS 侧的典型访问路径为：

- 字符设备：通过 `DeviceOps::get_chrdev_driver(dev)` 获取 `CharDriver`，再进行读写/ioctl
- 块设备：通过 `DeviceOps::get_blkdev_index(dev)` 将设备号映射到后端索引，再通过 `read_block`/`write_block` 等访问
