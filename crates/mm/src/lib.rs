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
//!
//! # 典型初始化顺序（由 os crate 驱动）
//!
//! 该 crate 本身不负责“发现可用物理内存”或“切换到某个页表”，这些通常由 `os` crate 完成。
//! 一般初始化流程如下：
//!
//! 1. `register_arch_ops(...)`：注册地址转换、TLB 批处理等架构相关操作
//! 2. `register_config(...)`：注册页大小/内核映射等内存布局常量
//! 3. `frame_allocator::init_frame_allocator(start, end)`：初始化物理帧分配器
//!
//! 随后即可构建页表与地址空间（[`page_table`] / [`memory_space`]）。

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

pub use arch_ops::{
    ArchMmOps, TlbBatchContextTrait, TlbBatchContextWrapper, arch_ops, register_arch_ops,
};
pub use config::{MmConfig, mm_config, register_config};
pub use file::{MmFile, MmInode};

// Re-export 常用类型
pub use address::{AlignOps, Paddr, PageNum, Ppn, PpnRange, UsizeConvert, Vaddr, Vpn, VpnRange};
pub use frame_allocator::{
    FrameRangeTracker, FrameTracker, TrackedFrames, alloc_contig_frames, alloc_frame, alloc_frames,
};
pub use memory_space::{AreaType, MapType, MappingArea, MemorySpace, MmapFile};
pub use page_table::{
    PageSize, PageTableEntry, PageTableInner, PagingError, PagingResult, UniversalPTEFlag,
};
