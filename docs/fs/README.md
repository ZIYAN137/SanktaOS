# FS 概览

FS（文件系统实现层）为内核提供具体文件系统，通过实现 VFS 的 `FileSystem` / `Inode` trait 接入统一的 VFS 抽象。

## 支持的文件系统（代码为准）

- `crates/fs/src/tmpfs/`：Tmpfs（内存文件系统）
- `crates/fs/src/proc/`：ProcFS（`/proc` 进程/系统信息导出）
- `crates/fs/src/sysfs/`：SysFS（`/sys` 设备/内核信息导出）
- `crates/fs/src/ext4/`：Ext4（基于 `ext4_rs` 的 ext4 读写支持）

## 运行时依赖（FsOps）

FS 通过 `crates/fs/src/ops.rs` 中的 `FsOps` 从 `os` 运行时获取页大小、时间、任务信息、挂载信息等数据，用于：

- tmpfs：页大小/时间
- procfs：任务、内存、挂载信息（生成 `/proc/*`）
- sysfs：设备注册表信息（生成 `/sys/*`）

## 文档位置

阶段迁移后，FS 的 API 参考与实现细节以源码 rustdoc 为准（建议通过 `cargo doc` 查看）。

