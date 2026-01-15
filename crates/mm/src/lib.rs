//! 内存管理子系统
//!
//! 提供地址抽象、物理帧分配、页表管理和内存空间管理功能。
//!
//! # 架构解耦
//!
//! 通过 trait 抽象与架构特定组件解耦：
//! - [`ArchMmOps`]: 地址转换、TLB 操作
//! - [`MmConfig`]: 内存布局常量
//!
//! 使用前必须调用 [`register_arch_ops`] 和 [`register_config`] 注册实现。

#![no_std]
#![feature(allocator_api)]

extern crate alloc;

mod arch_ops;
mod config;
mod file;

pub mod address;
pub mod frame_allocator;
pub mod memory_space;
pub mod page_table;

pub use arch_ops::{arch_ops, register_arch_ops, ArchMmOps, TlbBatchContextTrait, TlbBatchContextWrapper};
pub use config::{mm_config, register_config, MmConfig};
pub use file::{MmFile, MmInode};

// Re-export 常用类型
pub use address::{AlignOps, PageNum, Paddr, Ppn, PpnRange, UsizeConvert, Vaddr, Vpn, VpnRange};
pub use frame_allocator::{
    alloc_contig_frames, alloc_frame, alloc_frames, FrameRangeTracker, FrameTracker, TrackedFrames,
};
pub use memory_space::{AreaType, MapType, MappingArea, MemorySpace, MmapFile};
pub use page_table::{
    PageSize, PageTableEntry, PageTableInner, PagingError, PagingResult, UniversalPTEFlag,
};
