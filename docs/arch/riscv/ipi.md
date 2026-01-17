# 核间中断（IPI）

IPI（Inter-Processor Interrupt）用于 CPU 之间通信，常见用途包括：
- 跨核唤醒/重新调度（reschedule IPI）
- TLB shootdown（页表更新后通知其它核刷新 TLB）
- 停机/停止核（stop）

实现细节以源码 rustdoc 注释为准。

## 源码导览（以实现为准）

- IPI 逻辑：`os/src/arch/riscv/ipi.rs`
- SBI 发送 IPI：`os/src/arch/riscv/lib/sbi.rs`（`send_ipi`）
- Trap 分发到 IPI：`os/src/arch/riscv/trap/trap_handler.rs`（软件中断 `Trap::Interrupt(1)` → `handle_ipi()`）
- 调度唤醒触发：`os/src/kernel/scheduler/mod.rs`（跨核唤醒时发送 reschedule IPI）
- TLB 刷新触发：`os/src/arch/riscv/mm/`（页表更新后广播 tlb flush IPI）

## 调试提示

- IPI 收不到：确认已启用软件中断（SIE/SSIE），以及 `sip` 的 SSIP 位清除/置位逻辑是否正常
- 收到但不调度：确认 `handle_ipi()` 的 Reschedule 路径与调度器触发点（当前实现可能在中断返回路径统一调度）

