//! RISC-V 架构的陷阱（Trap）处理模块
//!
//! 该模块提供 RISC-V 的 trap 入口、上下文保存/恢复、以及与系统调用/信号返回等路径的集成。
//!
//! # 处理链路（概览）
//!
//! - 初始化：[`init_boot_trap`] / [`init`] 设置 `stvec` 指向汇编入口（`boot_trap_entry` / `trap_entry`）。
//! - 入口汇编：`trap_entry.S` 在内核栈上构造并保存 [`TrapFrame`]，随后调用 `trap_handler`（见 `trap_handler.rs`）。
//! - Rust 分发：`trap_handler` 读取 `scause/sepc/stval` 等信息，按异常/中断/系统调用进行分发；
//!   系统调用会进一步交给 `os/src/arch/riscv/syscall/mod.rs`。
//! - 恢复返回：[`restore`] 进入汇编 `__restore` 并执行 `sret` 返回。
//!
//! # 注意事项
//!
//! - Trap 处理中可能触发调度（例如用户态时钟中断）：返回时需要恢复“当前任务”的 TrapFrame，而不一定是入口参数。
//! - `SumGuard` 用于在内核态临时打开/关闭用户空间访问（SUM 位），供系统调用与 copyin/copyout 使用。
mod sum_guard;
mod trap_frame;
mod trap_handler;

use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    stvec::{self, Stvec},
};

pub use sum_guard::SumGuard;
pub use trap_frame::TrapFrame;

global_asm!(include_str!("trap_entry.S"));
global_asm!(include_str!("boot_trap_entry.S"));
global_asm!(include_str!("sigreturn.S"));

/// 初始化引导时的陷阱处理程序
pub fn init_boot_trap() {
    set_boot_trap_entry();
}

/// 初始化陷阱处理程序
pub fn init() {
    set_trap_entry();
    // 启用软件中断（用于 IPI）
    unsafe {
        crate::arch::intr::enable_software_interrupt();
    }
}

/// 恢复到陷阱前的上下文
/// # Safety
/// 该函数涉及直接操作处理器状态，必须确保传入的 TrapFrame 是有效且正确的。
pub unsafe fn restore(trap_frame: &TrapFrame) {
    unsafe { __restore(trap_frame) };
}

/// 获取信号返回的 trampoline 地址
pub fn sigreturn_trampoline_address() -> usize {
    crate::config::USER_SIGRETURN_TRAMPOLINE
}

/// Kernel-side instruction bytes for the rt_sigreturn trampoline.
///
/// These bytes are copied into a userspace RX page at `sigreturn_trampoline_address()`.
pub fn kernel_sigreturn_trampoline_bytes() -> &'static [u8] {
    let start = __sigreturn_trampoline as usize;
    let end = __sigreturn_trampoline_end as usize;
    let len = end.saturating_sub(start);
    unsafe { core::slice::from_raw_parts(start as *const u8, len) }
}

/// 设置 TrapFrame 中与当前 CPU 相关的字段。
///
/// RISC-V 的 trap_entry 依赖 `TrapFrame.cpu_ptr` 来恢复内核态的 tp。
///
/// # Safety
/// `trap_frame_ptr` 必须指向有效、可写且对齐的 TrapFrame。
#[inline]
pub unsafe fn set_trap_frame_cpu_ptr(trap_frame_ptr: *mut TrapFrame, cpu_ptr: usize) {
    // Safety: 由调用者保证指针有效
    unsafe {
        let tf = trap_frame_ptr
            .as_mut()
            .expect("set_trap_frame_cpu_ptr: null TrapFrame");
        tf.cpu_ptr = cpu_ptr;
    }
}

fn set_trap_entry() {
    // Safe: 仅在内核初始化阶段调用，确保唯一性
    unsafe {
        stvec::write(Stvec::new(trap_entry as usize, TrapMode::Direct));
    }
}

fn set_boot_trap_entry() {
    // Safe: 仅在内核初始化阶段调用，确保唯一性
    unsafe {
        stvec::write(Stvec::new(boot_trap_entry as usize, TrapMode::Direct));
    }
}

unsafe extern "C" {
    unsafe fn boot_trap_entry();
    unsafe fn trap_entry();
    unsafe fn __restore(trap_frame: &TrapFrame);
    unsafe fn __sigreturn_trampoline();
    unsafe fn __sigreturn_trampoline_end();
}
