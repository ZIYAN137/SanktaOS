#![allow(dead_code)]
//! 内存管理模块
//!
//! 本模块为内核提供与体系结构无关的内存管理抽象和实现。
//! 基础类型（address、page_table、frame_allocator）由独立的 `mm` crate 提供，
//! memory_space 和 global_allocator 模块保留在 os crate 中，因为它们包含 os-specific 的功能。

// Re-export mm crate 的公共 API
pub use mm::address;
pub use mm::frame_allocator;
pub use mm::page_table;

pub use mm::frame_allocator::init_frame_allocator;

// os-specific 的模块
pub mod global_allocator;
pub mod memory_space;

// Re-export global_allocator 中的 init_heap
pub use global_allocator::init_heap;

// Re-export memory_space 中的常用类型
pub use memory_space::{
    MemorySpace, get_global_kernel_space, kernel_root_ppn, kernel_token, with_kernel_space,
};
// 从 mm crate 重新导出 mapping_area 类型
pub use mm::memory_space::{AreaType, MapType, MappingArea, MmapFile};

use crate::arch::mm::vaddr_to_paddr;
use crate::config::{MEMORY_END, PAGE_SIZE};
use crate::earlyprintln;
use mm::address::{Ppn, UsizeConvert};

unsafe extern "C" {
    // 链接器脚本中定义的内核结束地址
    fn ekernel();
}

/// 初始化内存管理子系统
///
/// 此函数执行所有内存管理组件的初始化工作：
/// 1. 初始化物理帧分配器。
/// 2. 初始化内核堆分配器。
/// 3. 内核地址空间由 lazy_static KERNEL_SPACE 自动创建。
///
/// # 返回值
/// 返回内核根页表的 PPN，调用者需要在合适时机激活它。
pub fn init() -> Ppn {
    // 1. 初始化物理帧分配器

    // ekernel 是一个虚拟地址，需要转换为物理地址，以确定可分配物理内存的起始点。
    let ekernel_paddr = unsafe { vaddr_to_paddr(ekernel as usize) };

    // 计算页对齐后的物理内存起始地址。
    // 分配器将管理 [start, end) 范围内的内存。
    let start = ekernel_paddr.div_ceil(PAGE_SIZE) * PAGE_SIZE; // 页对齐

    // 优先使用设备树中的内存信息，否则使用配置中的 MEMORY_END
    let end = if let Some((dram_start, dram_size)) =
        crate::device::device_tree::early_dram_info()
    {
        let dram_end = dram_start.saturating_add(dram_size);
        earlyprintln!(
            "[MM] Using DRAM from device tree: {:#X} - {:#X} (size: {:#X})",
            dram_start,
            dram_end,
            dram_size
        );
        dram_end
    } else {
        earlyprintln!("[MM] Using MEMORY_END from config: {:#X}", MEMORY_END);
        MEMORY_END
    };

    // 初始化物理帧分配器
    init_frame_allocator(start, end);

    // 2. 初始化堆分配器
    init_heap();

    // 3. 内核地址空间由 lazy_static KERNEL_SPACE 自动创建
    // 这里只需要获取根页表 PPN
    let root_ppn = kernel_root_ppn();
    earlyprintln!(
        "[MM] Created kernel space, root PPN: 0x{:x}",
        root_ppn.as_usize()
    );
    root_ppn
}

/// 激活指定的地址空间
///
/// 通过将根页表（Page Table Root）的物理页号写入特定的寄存器，
/// 从而切换当前 CPU 使用的地址空间。
///
/// # 参数
///
/// * `root_ppn` - 新地址空间的根页表的物理页号。
pub fn activate(root_ppn: Ppn) {
    use mm::page_table::PageTableInner as PageTableInnerTrait;
    // 调用特定架构的页表激活函数，例如在 RISC-V 上设置 SATP 寄存器。
    crate::arch::mm::PageTableInner::activate(root_ppn);
}
