# 系统调用概述

系统调用（syscall）是用户态进入内核的主要入口。SanktaOS 的系统调用实现以“代码可追溯”为主：
在 mdBook 中只保留概览与索引，具体语义与边界条件以源码 rustdoc/实现为准。

## 入口与分发链路（以实现为准）

以 RISC-V 为例，系统调用的大致路径是：

1. 用户态执行 `ecall`，进入 trap 处理路径
2. `os/src/arch/riscv/syscall/mod.rs` 根据 `a7`（syscall number）分发到对应 `sys_*` 包装函数
3. `impl_syscall!` 生成的 `sys_*` 从 TrapFrame 提取 `a0..a5`，调用内核实现函数，并把返回值写回 `a0`
4. 真正逻辑位于 `os/src/kernel/syscall/*.rs`

LoongArch64 有对应的 `os/src/arch/loongarch/syscall/mod.rs` 分发实现。

## 源码导览

- 系统调用实现入口与“注册表”：`os/src/kernel/syscall/mod.rs`
  - 按功能拆分到 `io/fs/mm/ipc/network/signal/task/sys/...` 等文件
- 系统调用号（RISC-V）：`os/src/arch/riscv/syscall/syscall_number.rs`
- 分发与 `impl_syscall!`（RISC-V）：`os/src/arch/riscv/syscall/mod.rs`
- UAPI 类型与常量（errno、flag、struct 等）：`crates/uapi/src/`

## 返回值与 errno 约定

- 内核侧系统调用实现通常返回 `isize`：
  - 成功：返回非负值（如读写字节数、fd、0）
  - 失败：返回 `-(errno as isize)`（例如 `-(EINVAL as isize)`）
- errno 常量定义在 `crates/uapi/src/errno.rs`

## 用户指针与安全访问（建议）

系统调用经常需要读写用户空间指针。推荐做法：
- 先做参数与指针合法性检查（长度、NULL、范围、对齐等）
- 使用 `SumGuard` / `UserBuffer` / `validate_user_ptr*` 等封装进行用户内存访问
- 对 `unsafe` 代码块补充 `// SAFETY:` 说明（保持与当前代码风格一致）

## 如何新增/调整一个 syscall（实践步骤）

1. 实现内核函数：在 `os/src/kernel/syscall/<area>.rs` 添加 `pub fn xxx(...) -> isize`
2. 生成包装器：在 `os/src/kernel/syscall/mod.rs` 增加一条 `impl_syscall!(sys_xxx, xxx, (...))`
3. 分配系统调用号并接线：
   - RISC-V：在 `os/src/arch/riscv/syscall/syscall_number.rs` 增加常量，并在 `os/src/arch/riscv/syscall/mod.rs` 的 match 中加入分发分支
   - LoongArch64：在对应架构目录做同样的接线
4. 补齐 UAPI：如需新增结构体/flag/常量，把定义放到 `crates/uapi/src/`（供内核与用户态复用）
5. 补文档：把关键语义写在源码 rustdoc 注释里（而不是扩充 mdBook 的大清单）

