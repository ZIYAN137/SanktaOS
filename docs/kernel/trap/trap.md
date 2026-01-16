# 中断与陷阱概述

中断/陷阱（Trap）用于处理异常、中断与系统调用，是用户态与内核交互、以及内核响应硬件事件的关键机制。

本目录只保留概览类文档；实现细节与边界条件以源码 rustdoc 注释为准。

## Trap 的类型（概念）

以 RISC-V 为例，Trap 大致分为三类：
- **异常（Exception）**：同步事件，例如非法指令、页错误等
- **中断（Interrupt）**：异步事件，例如时钟中断、外部设备中断、核间中断（IPI）等
- **系统调用（System Call）**：用户态通过 `ecall` 主动陷入内核请求服务

## 入口与分发链路（以实现为准）

以 RISC-V 为例，Trap 处理的大致路径是：

1. **初始化向量**：`os/src/arch/riscv/trap/mod.rs` 设置 `stvec` 指向汇编入口（`trap_entry` / `boot_trap_entry`）
2. **保存现场**：`os/src/arch/riscv/trap/trap_entry.S` 在内核栈上构造并填充 `TrapFrame`，然后调用 `trap_handler`
3. **Rust 分发**：`os/src/arch/riscv/trap/trap_handler.rs` 读取 `scause/sepc/stval` 等信息，按来源（U/S）与原因码分发：
   - 系统调用：转到 `os/src/arch/riscv/syscall/mod.rs` 进行 syscall 分发
   - 时钟/外部/软件中断：驱动计时、设备中断派发、IPI 处理等
4. **可能发生调度**：例如定时器中断或 IPI 可能触发 `schedule()`；此时“返回时要恢复的 TrapFrame”可能已属于新任务
5. **恢复与返回**：`os/src/arch/riscv/trap/mod.rs::restore()` 进入汇编恢复路径并执行 `sret` 返回到用户态或内核态

LoongArch64 有对应的 arch 目录实现；整体思路相同，但入口/寄存器细节以对应源码为准。

## 与其他子系统的交互（概览）

- **系统调用**：用户态参数通过 TrapFrame 的参数寄存器传入；返回值写回 `a0`（以架构约定为准）
- **调度器**：时钟中断/IPI 等路径可能触发调度；调度相关逻辑主要在 `os/src/kernel/scheduler/` 与 `os/src/kernel/task/`
- **信号**：返回用户态前可能检查并投递信号（见 `os/src/ipc/signal.rs`）
- **设备中断**：外部中断通常经由中断控制器驱动与 `IRQ_MANAGER` 派发到具体驱动（见 `os/src/device/irq/`）

