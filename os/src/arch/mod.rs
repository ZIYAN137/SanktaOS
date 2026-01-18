//! 体系结构相关的模块
//!
//! 包含与特定处理器架构相关的实现。
//! 根据目标架构选择性地包含不同的子模块。
//!
//! # 分层约定
//!
//! 为了减少在 `arch/` 之外散落的 `cfg(target_arch = ...)` 与架构特定依赖：
//! - **架构条件编译应尽量集中在本模块**（选择 `riscv/` 或 `loongarch/`）。
//! - `arch/` 外部代码应通过 `crate::arch::*` 暴露的统一接口/钩子访问架构差异，
//!   避免直接依赖 `riscv`、`loongArch64` 等架构专用 crate 或寄存器操作。

#[cfg(target_arch = "loongarch64")]
mod loongarch;

#[cfg(target_arch = "riscv64")]
mod riscv;

// 导出架构特定的子模块
#[cfg(target_arch = "loongarch64")]
pub use loongarch::{
    boot, constant, info, intr, ipi, kernel, lib, mm, platform, syscall, timer, trap,
};

#[cfg(target_arch = "riscv64")]
pub use riscv::{boot, constant, info, intr, ipi, kernel, lib, mm, platform, syscall, timer, trap};

/// sync crate 的 ArchOps 实现
struct SyncArchOps;

impl sync::ArchOps for SyncArchOps {
    unsafe fn read_and_disable_interrupts(&self) -> usize {
        unsafe { self::intr::read_and_disable_interrupts() }
    }

    unsafe fn restore_interrupts(&self, flags: usize) {
        unsafe { self::intr::restore_interrupts(flags) }
    }

    fn sstatus_sie(&self) -> usize {
        self::constant::SSTATUS_SIE
    }

    fn cpu_id(&self) -> usize {
        self::kernel::cpu::cpu_id()
    }

    fn max_cpu_count(&self) -> usize {
        unsafe { crate::kernel::NUM_CPU }
    }
}

/// 全局 ArchOps 实例
static SYNC_ARCH_OPS: SyncArchOps = SyncArchOps;

/// 初始化 sync crate 的架构操作
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn init_sync_arch_ops() {
    unsafe { sync::register_arch_ops(&SYNC_ARCH_OPS) };
}
