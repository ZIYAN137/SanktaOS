# VFS 概览

VFS（Virtual File System）为内核提供统一的文件访问抽象，屏蔽不同文件系统/设备/管道等后端差异。

## 设计要点

- **分层抽象**：`File`（会话层，有状态）与 `Inode`（存储层，无状态）分离
- **路径与缓存**：路径解析、目录项缓存、挂载点跟随
- **设备与文件锁**：设备文件访问抽象、POSIX advisory locks（`fcntl`）

详细设计见：[整体架构](./architecture.md)

## 代码位置（以 rustdoc 为准）

- `crates/vfs/src/`：VFS 核心（File/Inode/Dentry/FDTable/路径解析/挂载/文件锁/设备号等）
- `os/src/kernel/syscall/`：系统调用对接（open/read/write/fcntl 等）

