# 设备号（devno）与设备映射

## 概述

VFS 使用 `u64` 表示设备号（dev），并提供：

- 设备号编码/解码工具（`makedev` / `major` / `minor`）
- 常用主设备号/次设备号常量（`chrdev_major` / `blkdev_major` / `misc_minor`）
- 从设备号到驱动/后端的映射辅助（`get_chrdev_driver` / `get_blkdev_index`）

相关源码：

- `crates/vfs/src/dev.rs`
- `crates/vfs/src/devno.rs`

## 设备号编码与解码（dev.rs）

VFS 采用 Linux 兼容格式：高 32 位为 major，低 32 位为 minor。

```rust
pub const fn makedev(major: u32, minor: u32) -> u64;
pub const fn major(dev: u64) -> u32;
pub const fn minor(dev: u64) -> u32;
```

## 常用设备号常量（devno.rs）

### 字符设备 major

```rust
pub mod chrdev_major {
    pub const MEM: u32 = 1;      // /dev/null, /dev/zero 等
    pub const TTY: u32 = 4;      // /dev/tty*, /dev/ttyS*
    pub const CONSOLE: u32 = 5;  // /dev/console
    pub const MISC: u32 = 10;    // /dev/misc/*
    pub const INPUT: u32 = 13;   // /dev/input/*
}
```

### MISC minor

```rust
pub mod misc_minor {
    pub const RTC: u32 = 135;
}
```

### 块设备 major

```rust
pub mod blkdev_major {
    pub const LOOP: u32 = 7;        // /dev/loop*
    pub const SCSI_DISK: u32 = 8;   // /dev/sd*
    pub const VIRTIO_BLK: u32 = 254; // /dev/vd*
}
```

## 设备号到后端的映射

`devno` 模块提供“从 dev -> 驱动/索引”的辅助函数，用于字符/块设备文件实现。

### 字符设备：get_chrdev_driver

```rust
pub fn get_chrdev_driver(dev: u64) -> Option<Arc<dyn CharDriver>>;
```

- 实际查找由 `DeviceOps::get_chrdev_driver(dev)` 完成（`devno` 仅做薄封装）。

### 块设备：get_blkdev_index

```rust
pub fn get_blkdev_index(dev: u64) -> Option<usize>;
```

当前采用简化的硬编码规则：

- `blkdev_major::VIRTIO_BLK`：`minor` 直接映射到块设备驱动数组索引
- `blkdev_major::SCSI_DISK`：每块磁盘占用 16 个 minor，索引为 `minor / 16`
- `blkdev_major::LOOP`：暂不支持，返回 `None`

## 使用示例

```rust
use vfs::dev::makedev;
use vfs::devno::{chrdev_major, misc_minor};

let dev_console = makedev(chrdev_major::CONSOLE, 1);
let dev_rtc = makedev(chrdev_major::MISC, misc_minor::RTC);
```

