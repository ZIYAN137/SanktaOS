# FS 子系统 API 参考

## 概述

本文档补充 `fs` crate 的运行时对接接口（`FsOps` / `TaskInfo`）以及 `procfs`、`sysfs` 相关的辅助 API，便于在 `os` crate 中完成 FS 层与内核运行时的解耦对接。

相关源码：

- `crates/fs/src/ops.rs`
- `crates/fs/src/proc/`
- `crates/fs/src/sysfs/`

## FsOps：FS 运行时操作接口

`FsOps` 抽象了 FS 层需要从内核运行时获取的能力（配置、时间、任务信息、系统信息等）。`os` crate 需要实现该 trait，并在启动早期注册。

### trait FsOps

配置：

```rust
fn page_size(&self) -> usize;
fn ext4_block_size(&self) -> usize;
fn fs_image_size(&self) -> usize;
fn virtio_blk_sector_size(&self) -> usize;
```

时间：

```rust
fn timespec_now(&self) -> TimeSpec;
```

任务与进程信息（供 `/proc` 使用）：

```rust
fn get_task(&self, pid: u32) -> Option<Arc<dyn TaskInfo>>;
fn list_process_pids(&self) -> Vec<u32>;
fn current_task_pid(&self) -> u32;
```

系统信息（供 `/proc` 使用）：

```rust
fn get_uptime_ms(&self) -> u64;
fn get_total_frames(&self) -> usize;
fn get_free_frames(&self) -> usize;
fn proc_cpuinfo(&self) -> Vec<u8>;
fn list_mounts(&self) -> Vec<MountInfo>;
```

### 注册与获取

```rust
pub unsafe fn register_fs_ops(ops: &'static dyn FsOps);
pub fn fs_ops() -> &'static dyn FsOps;
```

- `register_fs_ops`：必须在单线程环境下调用且只能调用一次（fat pointer 拆分存储）。
- `fs_ops`：获取已注册实现；若未注册则 panic。

## TaskInfo：任务信息接口

`TaskInfo` 为 `/proc/[pid]/*` 等 procfs 内容生成提供统一的任务信息访问接口。

```rust
pub trait TaskInfo: Send + Sync {
    fn pid(&self) -> u32;
    fn tid(&self) -> u32;
    fn ppid(&self) -> u32;
    fn pgid(&self) -> u32;

    fn name(&self) -> String;
    fn state(&self) -> TaskState;

    fn exe_path(&self) -> Option<String>;
    fn cmdline(&self) -> Vec<String>;

    fn vm_stats(&self) -> Option<VmStats>;
    fn memory_areas(&self) -> Vec<MemoryAreaInfo>;

    fn utime(&self) -> u64;
    fn stime(&self) -> u64;
    fn num_threads(&self) -> usize;
    fn start_time(&self) -> u64;
}
```

## 数据结构

### MountInfo

用于 `/proc/mounts` 生成：

- `device: String`：设备名（空字符串会被渲染为 `none`）
- `path: String`：挂载路径
- `fs_type: String`：文件系统类型
- `read_only: bool`：挂载是否只读

### TaskState

任务状态枚举，提供：

- `to_char()`：转换为 `/proc` 风格的状态字符（如 `R/S/D/T/Z`）
- `name()`：转换为状态名称字符串（如 `running/sleeping/...`）

### VmStats / MemoryAreaInfo

- `VmStats`：虚拟内存统计（text/rodata/data/bss/heap/stack/mmap/rss 等），并提供 `vm_size_kb()` / `rss_kb()`。
- `MemoryAreaInfo`：用于 `/proc/[pid]/maps` 的内存区域信息（地址范围、权限、偏移、设备号、inode、路径等）。

## SysFS：设备注册表辅助 API

`sysfs` 通过设备注册表辅助模块收集设备信息，并用于构建 `/sys/class/*`、`/sys/devices/*` 等树结构。

相关源码：

- `crates/fs/src/sysfs/device_registry.rs`
- `crates/fs/src/sysfs/builders/`

### 设备枚举与查询

块设备：

- `list_block_devices() -> Vec<BlockDeviceInfo>`
- `find_block_device(name: &str) -> Option<BlockDeviceInfo>`

网络设备：

- `list_net_devices() -> Vec<NetworkDeviceInfo>`
- `find_net_device(name: &str) -> Option<NetworkDeviceInfo>`

其他设备：

- `list_tty_devices() -> Vec<TtyDeviceInfo>`
- `list_input_devices() -> Vec<InputDeviceInfo>`
- `list_rtc_devices() -> Vec<RtcDeviceInfo>`

### SysFS 构建器（builders）

目前提供的典型构建入口：

- `build_block_devices(root: &Arc<SysfsInode>)`：构建 `/sys/class/block/*` 及相关符号链接
- `build_net_devices(root: &Arc<SysfsInode>)`：构建 `/sys/class/net/*` 及相关符号链接

## ProcFS：内容生成器（generators）

`procfs` 通过一组 `ContentGenerator` 生成伪文件内容，常见生成器包括：

- 系统级：`MeminfoGenerator`、`CpuinfoGenerator`、`MountsGenerator`、`UptimeGenerator`、`PsmemGenerator`
- 进程级（`/proc/[pid]/*`）：`CmdlineGenerator`、`MapsGenerator`、`StatGenerator`、`StatusGenerator`

这些生成器会通过 `fs_ops()` 拉取运行时信息（例如 `get_total_frames()`、`get_task(pid)`、`list_mounts()` 等），并序列化为 Linux 风格的 `/proc` 文本内容。

