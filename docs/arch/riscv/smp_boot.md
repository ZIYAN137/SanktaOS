# 多核启动（SMP）

本页记录 RISC-V 平台上多核（SMP）启动的整体流程与关键入口，便于在调试“从核未启动/卡死/不调度”等问题时快速定位代码。

实现细节以源码 rustdoc 注释为准。

## 关键入口（以实现为准）

- 启动入口与主核初始化：`os/src/arch/riscv/boot/mod.rs`
- 汇编入口：`os/src/arch/riscv/boot/entry.S`
- 平台设备初始化与设备树探测：`os/src/arch/riscv/platform/virt.rs`（调用 device tree / PLIC / virtio-mmio 等初始化）
- IPI 与从核调度唤醒：`os/src/arch/riscv/ipi.rs`、`os/src/kernel/scheduler/mod.rs`

## 流程概览

典型启动顺序（概览）：
1. 主核进入汇编入口，完成最小环境准备并跳入 Rust 启动代码
2. 主核初始化内存管理、trap、中断控制器/设备树等平台基础设施
3. 主核在合适时机启动从核（例如设置启动标志并通过 SBI/平台机制唤醒从核）
4. 从核完成自身最小初始化（trap、页表/CPU 本地状态等），加入调度体系
5. 主核继续完成剩余初始化并进入正常调度

## 调试提示

- 从核不起来：优先检查平台启动入口与“从核唤醒标志/路径”（见 `os/src/arch/riscv/boot/mod.rs` 与 `entry.S`）
- IPI 不生效：检查软件中断使能与 `Trap::Interrupt(1)` 分发是否到达 `handle_ipi()`（见 `os/src/arch/riscv/trap/trap_handler.rs`）
- 唤醒/调度异常：检查 `os/src/kernel/scheduler/mod.rs` 的跨核唤醒幂等性与 IPI 触发逻辑

