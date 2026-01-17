//! LoongArch64 内存管理模块
//!
//! 本模块提供了针对 **LoongArch64 架构** 的内存管理实现，
//! 使用 **3 级页表** 进行虚拟地址到物理地址的转换。
//!
//! # 页表结构
//!
//! LoongArch64 支持 39 位虚拟地址空间，使用 3 级页表：
//! - **Level 2 (PUD)**: 页上级目录，索引 VA\[38:30\]
//! - **Level 1 (PMD)**: 页中级目录，索引 VA\[29:21\]
//! - **Level 0 (PT)**:  页表，索引 VA\[20:12\]
//!
//! # 地址空间
//!
//! - **用户空间**: `0x0000_0000_0000_0000` - `0x0000_FFFF_FFFF_FFFF`
//! - **内核空间**: `0x9000_0000_0000_0000` - `0xFFFF_FFFF_FFFF_FFFF`
//!
//! # 直接映射
//!
//! 本模块使用**直接映射 (direct mapping)** 进行地址转换：
//! - 虚拟地址起始: `0x9000_0000_0000_0000`
//! - **提取物理地址**: 虚拟地址 & `PADDR_MASK`
//! - **创建虚拟地址**: 物理地址 | `VADDR_START`

mod page_table;
mod page_table_entry;

pub use page_table::{PageTableInner, TlbBatchContext};
pub use page_table_entry::PageTableEntry;

use mm::{ArchMmOps, TlbBatchContextTrait, TlbBatchContextWrapper};

/// LoongArch64 直接映射窗口起始地址
///
/// 内核虚拟地址空间从此地址开始，通过 DMW (Direct Mapping Window) 配置。
pub const VADDR_START: usize = 0x9000_0000_0000_0000;

/// 物理地址掩码
///
/// 用于从虚拟地址提取物理地址，保留低 48 位。
pub const PADDR_MASK: usize = 0x0000_FFFF_FFFF_FFFF;

/// 虚拟地址转物理地址
///
/// # 参数
///
/// * `vaddr` - 虚拟地址（必须在直接映射区域内）
///
/// # 返回
///
/// 对应的物理地址
///
/// # Safety
///
/// 调用者必须确保虚拟地址在直接映射区域内。
#[inline]
pub const unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
    vaddr & PADDR_MASK
}

/// 物理地址转虚拟地址
///
/// # 参数
///
/// * `paddr` - 物理地址
///
/// # 返回
///
/// 对应的虚拟地址（在直接映射区域内）
#[inline]
pub const fn paddr_to_vaddr(paddr: usize) -> usize {
    paddr | VADDR_START
}

// ============ ArchMmOps trait 实现 ============

/// LoongArch 架构的内存管理操作实现
struct LoongArchMmOps;

impl ArchMmOps for LoongArchMmOps {
    unsafe fn vaddr_to_paddr(&self, vaddr: usize) -> usize {
        vaddr & PADDR_MASK
    }

    fn paddr_to_vaddr(&self, paddr: usize) -> usize {
        paddr | VADDR_START
    }

    fn sigreturn_trampoline_bytes(&self) -> &'static [u8] {
        crate::arch::trap::kernel_sigreturn_trampoline_bytes()
    }

    fn num_cpus(&self) -> usize {
        unsafe { crate::kernel::NUM_CPU }
    }

    fn send_tlb_flush_ipi_all(&self) {
        // LoongArch 多核 IPI 尚未实现，暂时只刷新本地 TLB
        // TODO: 实现 LoongArch 多核 TLB shootdown
    }

    fn create_tlb_batch_context(&self) -> TlbBatchContextWrapper {
        unsafe { TlbBatchContextWrapper::new(TlbBatchContext::new()) }
    }
}

impl TlbBatchContextTrait for TlbBatchContext {
    fn flush(&mut self) {
        TlbBatchContext::flush(self);
    }
}

static LOONGARCH_MM_OPS: LoongArchMmOps = LoongArchMmOps;

/// 注册 LoongArch 架构的内存管理操作
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_mm_ops() {
    mm::register_arch_ops(&LOONGARCH_MM_OPS);
}
