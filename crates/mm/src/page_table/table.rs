//! 页表内部结构模块
//!
//! 本模块定义了页表的内部接口，供不同架构的页表实现使用。
//! 通过该接口，可以实现对页表的创建、映射、解除映射、翻译等操作。
//!
//! ## 设计要点
//!
//! - `PageTableInner` 由各架构实现（例如不同 MMU、不同页表格式）。
//! - 上层（如 [`crate::memory_space::MemorySpace`]）只依赖该 trait，
//!   从而实现“地址空间管理逻辑”与“页表硬件细节”的解耦。
//!
//! ## TLB 刷新与批处理
//!
//! 映射/解除映射通常需要配合 TLB 刷新；为减少频繁刷新带来的开销，
//! 该接口提供 `*_with_batch` 版本，配合 [`crate::arch_ops::TlbBatchContextWrapper`]
//! 在一次批处理中合并刷新操作（具体行为由架构实现决定）。
#![allow(dead_code)]
use super::{PageSize, PageTableEntry, PagingResult, UniversalPTEFlag};
use crate::address::{Paddr, Ppn, Vaddr, Vpn};
use crate::arch_ops::TlbBatchContextWrapper;

/// 页表内部接口
///
/// 此 trait 定义了页表的核心操作，由具体架构实现。
pub trait PageTableInner<T>
where
    T: PageTableEntry,
{
    /// 页表级数
    const LEVELS: usize;
    /// 最大虚拟地址位数
    const MAX_VA_BITS: usize;
    /// 最大物理地址位数
    const MAX_PA_BITS: usize;

    /// 刷新单个 TLB 条目
    fn tlb_flush(vpn: Vpn);
    /// 刷新所有 TLB 条目
    fn tlb_flush_all();

    /// 检查是否为用户页表
    fn is_user_table(&self) -> bool;

    /// 激活页表
    fn activate(ppn: Ppn);
    /// 获取当前活动页表的根 PPN
    fn activating_table_ppn() -> Ppn;

    /// 创建新页表
    fn new() -> Self;
    /// 从 PPN 创建页表
    fn from_ppn(ppn: Ppn) -> Self;
    /// 创建内核页表
    fn new_as_kernel_table() -> Self;

    /// 获取根页表的 PPN
    fn root_ppn(&self) -> Ppn;

    /// 获取指定级别的页表项
    fn get_entry(&self, vpn: Vpn, level: usize) -> Option<(T, PageSize)>;

    /// 翻译虚拟地址到物理地址
    fn translate(&self, vaddr: Vaddr) -> Option<Paddr>;

    /// 映射虚拟页到物理页
    fn map(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()>;

    /// 解除映射
    fn unmap(&mut self, vpn: Vpn) -> PagingResult<()>;

    /// 移动映射
    fn mvmap(
        &mut self,
        vpn: Vpn,
        target_ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
    ) -> PagingResult<()>;

    /// 更新映射标志
    fn update_flags(&mut self, vpn: Vpn, flags: UniversalPTEFlag) -> PagingResult<()>;

    /// 遍历页表获取映射信息
    fn walk(&self, vpn: Vpn) -> PagingResult<(Ppn, PageSize, UniversalPTEFlag)>;

    /// 映射虚拟页到物理页（支持 TLB 批处理）
    fn map_with_batch(
        &mut self,
        vpn: Vpn,
        ppn: Ppn,
        page_size: PageSize,
        flags: UniversalPTEFlag,
        batch: Option<&mut TlbBatchContextWrapper>,
    ) -> PagingResult<()>;

    /// 解除映射（支持 TLB 批处理）
    fn unmap_with_batch(
        &mut self,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContextWrapper>,
    ) -> PagingResult<()>;

    /// 更新映射标志（支持 TLB 批处理）
    fn update_flags_with_batch(
        &mut self,
        vpn: Vpn,
        flags: UniversalPTEFlag,
        batch: Option<&mut TlbBatchContextWrapper>,
    ) -> PagingResult<()>;
}
