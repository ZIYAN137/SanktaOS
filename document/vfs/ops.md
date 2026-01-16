# VFS 运行时接口（ops）说明

## 概述

VFS 通过 `VfsOps`/`DeviceOps` 两个 trait 将“与内核运行时强相关的操作”从 `vfs` crate 中抽象出去，从而实现与 `os` crate 的解耦。`os` crate 负责实现并在启动早期注册这些接口。

相关源码：

- `crates/vfs/src/ops.rs`

## VfsOps：VFS 运行时操作

`VfsOps` 主要覆盖“任务上下文、配置、时间、控制台与用户空间访问保护”等能力。

```rust
pub trait VfsOps: Send + Sync {
    // 任务上下文
    fn current_cwd(&self) -> Option<Arc<Dentry>>;
    fn current_root(&self) -> Option<Arc<Dentry>>;

    // 配置
    fn default_max_fds(&self) -> usize;

    // 时间
    fn timespec_now(&self) -> TimeSpec;

    // 用户空间访问保护（SUM 等机制的抽象）
    fn enter_user_access(&self);
    fn exit_user_access(&self);

    // 控制台
    fn console_getchar(&self) -> Option<u8>;
    fn console_putchar(&self, c: u8);
    fn console_write_str(&self, s: &str);
}
```

### 注册与获取

```rust
pub unsafe fn register_vfs_ops(ops: &'static dyn VfsOps);
pub fn vfs_ops() -> &'static dyn VfsOps;
```

- `register_vfs_ops`：必须在单线程环境下调用且只能调用一次。
- `vfs_ops`：获取已注册实现；若未注册则 panic。

### UserAccessGuard

`UserAccessGuard` 是对 `enter_user_access()` / `exit_user_access()` 的 RAII 封装：

```rust
pub struct UserAccessGuard;

impl UserAccessGuard {
    pub fn new() -> Self;
}
```

- 创建 guard 时进入用户空间访问模式；离开作用域时自动退出。
- 适用于需要临时允许访问用户地址空间的路径（例如系统调用读取/写回用户缓冲区）。

## DeviceOps：设备操作抽象

设备相关操作由 `DeviceOps` 抽象，VFS 侧通过它访问字符设备驱动与块设备后端。

### CharDriver：字符设备驱动接口

```rust
pub trait CharDriver: Send + Sync {
    fn try_read(&self) -> Option<u8>;
    fn write(&self, data: &[u8]);
    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, i32>;
}
```

### trait DeviceOps

```rust
pub trait DeviceOps: Send + Sync {
    fn get_chrdev_driver(&self, dev: u64) -> Option<Arc<dyn CharDriver>>;

    fn get_blkdev_index(&self, dev: u64) -> Option<usize>;
    fn read_block(&self, idx: usize, block_id: usize, buf: &mut [u8]) -> bool;
    fn write_block(&self, idx: usize, block_id: usize, buf: &[u8]) -> bool;
    fn blkdev_total_blocks(&self, idx: usize) -> usize;
}
```

### 注册与获取

```rust
pub unsafe fn register_device_ops(ops: &'static dyn DeviceOps);
pub fn device_ops() -> &'static dyn DeviceOps;
```

- `register_device_ops`：必须在单线程环境下调用且只能调用一次。
- `device_ops`：获取已注册实现；若未注册则 panic。

