# RISC-V 概述

本章记录 SanktaOS 在 RISC-V64 上的关键架构相关实现入口，便于定位代码与理解启动/中断/多核等机制。

说明：本目录只保留概览类文档；实现细节与边界条件以源码 rustdoc 注释为准。

## 源码导览（以实现为准）

- 启动入口：`os/src/arch/riscv/boot/`
- Trap（异常/中断/系统调用入口）：`os/src/arch/riscv/trap/`
- 系统调用分发：`os/src/arch/riscv/syscall/`
- 多核 IPI：`os/src/arch/riscv/ipi.rs`
- 中断使能与软中断：`os/src/arch/riscv/intr/`
- 内核态切换上下文：`os/src/arch/riscv/kernel/`
- 虚拟内存与页表：`os/src/arch/riscv/mm/`
- 平台初始化（virt）：`os/src/arch/riscv/platform/virt.rs`

## 用户栈布局（提示）

用户程序初始栈（argc/argv/envp/auxv 等）由 RISC-V 侧的栈布局构造逻辑生成：
- `os/src/arch/riscv/kernel/task.rs`（`setup_stack_layout`）
- `os/src/arch/riscv/trap/trap_frame.rs`（将 argc/argv/envp 写入 a0/a1/a2）

