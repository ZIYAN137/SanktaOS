//! IPI (Inter-Processor Interrupt) 核间中断
//!
//! 用于 CPU 间通信，支持调度唤醒、TLB 刷新等功能。
//!
//! # 设计说明
//!
//! - 使用 sbi-rt crate 的 IPI 扩展发送中断
//! - Per-CPU 原子标志位存储待处理的 IPI 类型
//! - 在软件中断处理程序中处理 IPI
//!
//! # 性能考虑
//!
//! - 中断上下文不进行内存分配
//! - 使用原子操作避免锁竞争
//! - 支持批量发送减少 SBI 调用次数

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::config::MAX_CPU_COUNT;

/// IPI 类型
///
/// 使用位标志表示，支持组合多种 IPI 类型
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum IpiType {
    /// 重新调度（通知目标 CPU 有新任务）
    Reschedule = 1 << 0,
    /// TLB 刷新（页表更新后同步）
    TlbFlush = 1 << 1,
    /// 停止 CPU（系统关机）
    Stop = 1 << 2,
}

/// Per-CPU 待处理 IPI 标志
///
/// 每个 CPU 一个原子变量，存储待处理的 IPI 类型位掩码
static IPI_PENDING: [AtomicU32; MAX_CPU_COUNT] = [const { AtomicU32::new(0) }; MAX_CPU_COUNT];

// ============================================================================
// TLB shootdown acknowledgements (SMP correctness)
// ============================================================================
//
// Rationale:
// - Page table changes (especially unmap / permission drop) can free physical pages.
// - If we don't wait for other CPUs to flush their TLBs, a remote CPU may keep using
//   a stale translation to a page whose frame has been returned to the allocator and
//   potentially re-used, causing memory corruption.
//
// We implement a simple "coalescing" sequence protocol:
// - The sender assigns a monotonically increasing seq for each broadcast.
// - Each target CPU records the latest requested seq in TLB_FLUSH_REQ[cpu].
// - When handling the TlbFlush IPI, the CPU flushes and then sets TLB_FLUSH_ACK[cpu]
//   to the latest request value (thus coalescing multiple pending requests).
// - The sender waits until all target CPUs' ACK >= seq.
//
// This is intentionally minimal and does not require locks in interrupt context.
static TLB_FLUSH_GLOBAL_SEQ: AtomicU64 = AtomicU64::new(0);
static TLB_FLUSH_REQ: [AtomicU64; MAX_CPU_COUNT] = [const { AtomicU64::new(0) }; MAX_CPU_COUNT];
static TLB_FLUSH_ACK: [AtomicU64; MAX_CPU_COUNT] = [const { AtomicU64::new(0) }; MAX_CPU_COUNT];

/// 发送 IPI 到指定 CPU
///
/// # 参数
/// - target_cpu: 目标 CPU ID
/// - ipi_type: IPI 类型
///
/// # Panics
///
/// 如果 target_cpu >= NUM_CPU，会 panic
pub fn send_ipi(target_cpu: usize, ipi_type: IpiType) {
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    core::assert!(target_cpu < num_cpu, "Invalid target CPU: {}", target_cpu);

    // 1. 设置目标 CPU 的待处理标志
    IPI_PENDING[target_cpu].fetch_or(ipi_type as u32, Ordering::Release);

    // 2. 通过 SBI 触发软件中断
    let hart_mask = 1usize << target_cpu;
    crate::arch::lib::sbi::send_ipi(hart_mask);

    // 3. 检查目标CPU的sip寄存器（仅用于调试）
    if target_cpu == crate::arch::kernel::cpu::cpu_id() {
        // 如果是当前CPU，可以读取sip
        unsafe {
            let sip: usize;
            core::arch::asm!("csrr {}, sip", out(reg) sip);
            crate::pr_debug!(
                "[IPI] After send_ipi, current CPU sip={:#x}, SSIP bit: {}",
                sip,
                (sip >> 1) & 1
            );
        }
    }
}

/// 发送 IPI 到多个 CPU
///
/// # 参数
/// - hart_mask: hart 位掩码，每位代表一个 CPU
/// - ipi_type: IPI 类型
pub fn send_ipi_many(hart_mask: usize, ipi_type: IpiType) {
    let num_cpu = unsafe { crate::kernel::NUM_CPU };

    // 设置所有目标 CPU 的待处理标志
    for cpu in 0..num_cpu {
        if (hart_mask & (1 << cpu)) != 0 {
            IPI_PENDING[cpu].fetch_or(ipi_type as u32, Ordering::Release);
        }
    }

    // 一次性发送到所有目标 CPU
    crate::arch::lib::sbi::send_ipi(hart_mask);
}

/// 发送调度 IPI
///
/// 通知目标 CPU 有新任务需要调度
///
/// # 参数
/// - cpu: 目标 CPU ID
pub fn send_reschedule_ipi(cpu: usize) {
    send_ipi(cpu, IpiType::Reschedule);
}

/// 广播 TLB 刷新 IPI
///
/// 通知所有其他 CPU 刷新 TLB
pub fn send_tlb_flush_ipi_all() {
    let current_cpu_id = super::kernel::cpu::cpu_id();
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    let mask = ((1 << num_cpu) - 1) & !(1 << current_cpu_id);

    if mask != 0 {
        send_ipi_many(mask, IpiType::TlbFlush);
    }
}

/// Broadcast a TLB flush and wait for acknowledgements from all other CPUs.
///
/// This is used by page table operations that can remove permissions/unmap pages,
/// where freeing physical frames before remote TLBs are flushed is unsafe.
pub fn send_tlb_flush_ipi_all_sync() {
    let current_cpu_id = super::kernel::cpu::cpu_id();
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    let mask = ((1 << num_cpu) - 1) & !(1 << current_cpu_id);

    if mask == 0 {
        return;
    }

    // Monotonic seq starting from 1.
    let seq = TLB_FLUSH_GLOBAL_SEQ.fetch_add(1, Ordering::AcqRel) + 1;

    // Publish the requested seq for each target CPU before raising the IPI.
    for cpu in 0..num_cpu {
        if (mask & (1 << cpu)) != 0 {
            TLB_FLUSH_REQ[cpu].store(seq, Ordering::Release);
        }
    }

    send_ipi_many(mask, IpiType::TlbFlush);

    // Wait until all target CPUs have ACKed.
    //
    // Important: we may be called with interrupts disabled (e.g. in a trap or while holding
    // locks). To avoid "mutual wait" deadlocks (CPU A waits for CPU B, while CPU B waits for
    // CPU A), we opportunistically service incoming IPIs if SSIP is pending.
    while !tlb_flush_acked(mask, seq) {
        if ssip_pending() {
            handle_ipi();
        }
        core::hint::spin_loop();
    }
}

#[inline]
fn tlb_flush_acked(mask: usize, seq: u64) -> bool {
    let num_cpu = unsafe { crate::kernel::NUM_CPU };
    for cpu in 0..num_cpu {
        if (mask & (1 << cpu)) == 0 {
            continue;
        }
        if TLB_FLUSH_ACK[cpu].load(Ordering::Acquire) < seq {
            return false;
        }
    }
    true
}

#[inline]
fn ssip_pending() -> bool {
    let sip: usize;
    unsafe { core::arch::asm!("csrr {}, sip", out(reg) sip) };
    // SSIP is bit 1.
    (sip & (1 << 1)) != 0
}

/// 处理 IPI（在软件中断处理中调用）
///
/// 读取并清除当前 CPU 的待处理标志，执行相应操作
pub fn handle_ipi() {
    let cpu = super::kernel::cpu::cpu_id();

    // 清除 SSIP 位（软件中断挂起位）
    // SAFETY: 清除 sip 寄存器的 SSIP 位是安全的，这是标准的中断处理流程
    unsafe {
        core::arch::asm!("csrc sip, {}", in(reg) 1 << 1);
    }

    // 读取并清除待处理标志
    let pending = IPI_PENDING[cpu].swap(0, Ordering::AcqRel);

    if pending == 0 {
        return;
    }

    crate::pr_debug!("[IPI] CPU {} handling IPI: {:#x}", cpu, pending);

    // 处理调度 IPI
    if pending & (IpiType::Reschedule as u32) != 0 {
        crate::pr_debug!("[IPI] CPU {} received Reschedule IPI", cpu);
        // 调度将在中断返回时由 check_signal 后的逻辑处理
        // 这里只需要标记即可
    }

    // 处理 TLB 刷新 IPI
    if pending & (IpiType::TlbFlush as u32) != 0 {
        // SAFETY: sfence.vma 是安全的 RISC-V 指令，用于刷新 TLB
        unsafe {
            core::arch::asm!("sfence.vma");
        }
        // Ack the latest requested seq (coalescing multiple pending flushes).
        let seq = TLB_FLUSH_REQ[cpu].load(Ordering::Acquire);
        TLB_FLUSH_ACK[cpu].store(seq, Ordering::Release);
    }

    // 处理停止 IPI
    if pending & (IpiType::Stop as u32) != 0 {
        crate::pr_debug!("[IPI] CPU {} stopping", cpu);
        loop {
            // SAFETY: wfi 是安全的 RISC-V 指令，用于等待中断
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试 IPI 类型位标志
    #[test_case]
    fn test_ipi_type_flags() {
        assert!(IpiType::Reschedule as u32 == 1);
        assert!(IpiType::TlbFlush as u32 == 2);
        assert!(IpiType::Stop as u32 == 4);
    }

    // 测试 IPI 类型组合
    #[test_case]
    fn test_ipi_type_combination() {
        let combined = (IpiType::Reschedule as u32) | (IpiType::TlbFlush as u32);
        assert!(combined == 3);

        let all =
            (IpiType::Reschedule as u32) | (IpiType::TlbFlush as u32) | (IpiType::Stop as u32);
        assert!(all == 7);
    }

    // 测试 IPI 类型位检查
    #[test_case]
    fn test_ipi_type_bit_check() {
        let flags = (IpiType::Reschedule as u32) | (IpiType::Stop as u32);

        assert!(flags & (IpiType::Reschedule as u32) != 0);
        assert!(flags & (IpiType::TlbFlush as u32) == 0);
        assert!(flags & (IpiType::Stop as u32) != 0);
    }
}
