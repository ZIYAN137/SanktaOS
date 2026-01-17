//! 设备树（FDT）解析与设备探测
//!
//! 本模块负责从引导程序提供的设备树中解析平台信息，并按 `compatible` 触发各类设备的初始化。
//!
//! # 关键对象
//!
//! - `DTP`：由引导程序设置的 DTB 指针（物理地址经转换后用于解析）。
//! - `FDT`：解析后的设备树对象。
//! - `DEVICE_TREE_REGISTRY`：`compatible` → 探测函数的注册表，驱动通过各自的 `driver_init()` 注册。
//! - `DEVICE_TREE_INTC`：`phandle` → 中断控制器驱动的映射表（用于设备解析中断相关属性时查询）。
//!
//! # 两阶段初始化
//!
//! ## Phase 1: 早期解析（无堆分配）
//! `phase1_early_parse()` 在 `mm::init()` 之前调用，直接解析设备树二进制数据：
//! - 提取 CPU 数量、时钟频率
//! - 提取内存区域信息
//! - 存储到固定大小的静态数组（不使用堆分配）
//!
//! ## Phase 2: 完整初始化（可用堆分配）
//! `phase2_full_init()` 在 `mm::init()` 之后调用，执行完整的设备驱动初始化：
//! 1) 仅初始化 `interrupt-controller` 节点（例如 PLIC），保证后续设备注册中断时中断控制器已就绪；
//! 2) 初始化其余设备节点（例如 virtio-mmio、rtc、net 等）。

use crate::{
    device::{CMDLINE, irq::IntcDriver},
    kernel::{CLOCK_FREQ, NUM_CPU},
    mm::address::{ConvertablePaddr, Paddr, UsizeConvert},
    pr_info, pr_warn,
    sync::RwLock,
};
use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use fdt::{Fdt, node::FdtNode};
/// 指向设备树的指针，在启动时由引导程序设置
#[unsafe(no_mangle)]
pub static mut DTP: usize = 0x114514; // 占位地址，实际由引导程序设置

/// Phase 1 提取的内存区域信息（最多支持 8 个区域）
const MAX_MEMORY_REGIONS: usize = 8;

/// 内存区域描述
#[derive(Copy, Clone)]
struct MemoryRegion {
    start: usize,
    size: usize,
}

/// Phase 1 提取的早期设备树信息
struct EarlyDtInfo {
    /// CPU 核心数量
    num_cpus: usize,
    /// 时钟频率
    clock_freq: usize,
    /// 内存区域数量
    memory_region_count: usize,
    /// 内存区域列表
    memory_regions: [MemoryRegion; MAX_MEMORY_REGIONS],
}

impl EarlyDtInfo {
    const fn empty() -> Self {
        Self {
            num_cpus: 1,
            clock_freq: 12_500_000,
            memory_region_count: 0,
            memory_regions: [MemoryRegion { start: 0, size: 0 }; MAX_MEMORY_REGIONS],
        }
    }
}

/// Phase 1 提取的信息（静态存储，无需堆分配）
static mut EARLY_DT_INFO: EarlyDtInfo = EarlyDtInfo::empty();

lazy_static::lazy_static! {
    /// 设备树
    /// 通过 DTP 指针解析得到
    /// XXX: 是否需要这个?
    pub static ref FDT: Fdt<'static> = {
        unsafe {
            let addr = Paddr::to_vaddr(&Paddr::from_usize(DTP));
            fdt::Fdt::from_ptr(addr.as_usize() as *mut u8).expect("Failed to parse device tree")
        }
    };

    /// Compatible 字符串到探测函数的映射表
    /// 键为设备的 compatible 字符串，值为对应的探测函数
    /// 用于在设备树中查找和初始化设备
    pub static ref DEVICE_TREE_REGISTRY: RwLock<BTreeMap<&'static str, fn(&FdtNode)>> =
        RwLock::new(BTreeMap::new());

    /// 设备树中断控制器映射表
    /// 键为中断控制器的 phandle，值为对应的中断控制器驱动程序
    /// 用于在设备树中查找和管理中断控制器
    pub static ref DEVICE_TREE_INTC: RwLock<BTreeMap<u32, Arc<dyn IntcDriver>>> =
        RwLock::new(BTreeMap::new());
}

/// Phase 1: 早期设备树解析（无堆分配）
///
/// 在 mm::init() 之前调用，直接解析设备树二进制数据
/// 提取 CPU 数量、时钟频率、内存区域等关键信息
///
/// # Safety
/// 必须在单核环境下调用，且 DTP 已被正确设置
pub unsafe fn phase1_early_parse() {
    let dtb_ptr = Paddr::to_vaddr(&Paddr::from_usize(DTP)).as_usize() as *const u8;

    // 直接使用 fdt crate 的 no-alloc API 解析
    let fdt = match fdt::Fdt::from_ptr(dtb_ptr) {
        Ok(fdt) => fdt,
        Err(_) => {
            // 解析失败，使用默认值
            EARLY_DT_INFO = EarlyDtInfo::empty();
            return;
        }
    };

    // 提取 CPU 数量
    let num_cpus = fdt.cpus().count();
    EARLY_DT_INFO.num_cpus = if num_cpus > 0 { num_cpus } else { 1 };

    // 提取时钟频率
    if let Some(cpu) = fdt.cpus().next() {
        let timebase = cpu
            .property("timebase-frequency")
            .or_else(|| cpu.property("clock-frequency"))
            .and_then(|p| match p.value.len() {
                4 => Some(u32::from_be_bytes(p.value[..4].try_into().ok()?) as usize),
                8 => Some(u64::from_be_bytes(p.value[..8].try_into().ok()?) as usize),
                _ => None,
            });
        if let Some(freq) = timebase {
            EARLY_DT_INFO.clock_freq = freq;
        }
    }

    // 提取内存区域信息
    let mut count = 0;
    for region in fdt.memory().regions() {
        if count >= MAX_MEMORY_REGIONS {
            break;
        }
        let size = region.size.unwrap_or(0) as usize;
        if size > 0 {
            EARLY_DT_INFO.memory_regions[count] = MemoryRegion {
                start: region.starting_address as usize,
                size,
            };
            count += 1;
        }
    }
    EARLY_DT_INFO.memory_region_count = count;
}

/// 获取 Phase 1 提取的 CPU 数量
pub fn early_num_cpus() -> usize {
    unsafe { EARLY_DT_INFO.num_cpus }
}

/// 获取 Phase 1 提取的时钟频率
pub fn early_clock_freq() -> usize {
    unsafe { EARLY_DT_INFO.clock_freq }
}

/// 获取 Phase 1 提取的 DRAM 信息（起始地址和总大小）
pub fn early_dram_info() -> Option<(usize, usize)> {
    unsafe {
        if EARLY_DT_INFO.memory_region_count == 0 {
            return None;
        }

        let mut start = usize::MAX;
        let mut end = 0usize;

        for i in 0..EARLY_DT_INFO.memory_region_count {
            let region = &EARLY_DT_INFO.memory_regions[i];
            let s = region.start;
            let e = s.saturating_add(region.size);
            if s < start {
                start = s;
            }
            if e > end {
                end = e;
            }
        }

        if start < end {
            Some((start, end - start))
        } else {
            None
        }
    }
}

/// 早期初始化: 只解析 CPU 数量和时钟频率
///
/// 此函数在堆分配器初始化之前调用,因此不能使用任何需要堆分配的操作。
///
/// 注意：此函数已被 phase1_early_parse() 取代，保留仅为兼容性
#[deprecated(note = "使用 phase1_early_parse() 代替")]
pub fn early_init() {
    let cpus = FDT.cpus().count();
    // SAFETY: 这里是在单核初始化阶段设置 CPU 数量
    unsafe { NUM_CPU = cpus };

    if let Some(cpu) = FDT.cpus().next() {
        let timebase = cpu
            .property("timebase-frequency")
            .or_else(|| cpu.property("clock-frequency"))
            .and_then(|p| match p.value.len() {
                4 => Some(u32::from_be_bytes(p.value.try_into().ok()?) as usize),
                8 => Some(u64::from_be_bytes(p.value.try_into().ok()?) as usize),
                _ => None,
            });
        if let Some(freq) = timebase {
            unsafe {
                CLOCK_FREQ = freq;
            }
        } else {
            pr_warn!("[Device] No timebase-frequency in DTB, keeping default");
        }
    } else {
        pr_warn!("[Device] No CPU found in device tree");
    }
}

/// Phase 2: 完整设备树初始化（可使用堆分配）
///
/// 在 mm::init() 之后调用，执行完整的设备驱动初始化
pub fn phase2_full_init() {
    let model = FDT
        .root()
        .property("model")
        .and_then(|p| p.value.split(|b| *b == 0).next())
        .and_then(|s| core::str::from_utf8(s).ok())
        .unwrap_or("unknown");
    pr_info!("[Device] devicetree of {} is initialized", model);

    // 从 Phase 1 数据设置全局变量
    unsafe {
        NUM_CPU = EARLY_DT_INFO.num_cpus;
        CLOCK_FREQ = EARLY_DT_INFO.clock_freq;
    }

    pr_info!("[Device] now has {} CPU(s)", unsafe { NUM_CPU });
    pr_info!("[Device] CLOCK_FREQ set to {} Hz", unsafe { CLOCK_FREQ });

    // 打印内存区域（使用 Phase 1 数据）
    unsafe {
        for i in 0..EARLY_DT_INFO.memory_region_count {
            let region = &EARLY_DT_INFO.memory_regions[i];
            pr_info!(
                "[Device] Memory Region: Start = {:#X}, Size = {:#X}",
                region.start,
                region.size
            );
        }
    }

    if let Some(bootargs) = FDT.chosen().bootargs() {
        if !bootargs.is_empty() {
            pr_info!("Kernel cmdline: {}", bootargs);
            *CMDLINE.write() = String::from(bootargs);
        }
    }

    // 首先初始化中断控制器
    walk_dt(&FDT, true);
    walk_dt(&FDT, false);
}

/// 初始化设备树
///
/// 注意：此函数已被 phase2_full_init() 取代，保留仅为兼容性
pub fn init() {
    phase2_full_init();
}

/// 遍历设备树，查找并初始化 virtio 设备
/// # 参数
/// * `fdt` - 设备树对象
fn walk_dt(fdt: &Fdt, intc_only: bool) {
    for node in fdt.all_nodes() {
        if let Some(compatible) = node.compatible() {
            if node.property("interrupt-controller").is_some() == intc_only {
                pr_info!("[Device] Found device: {}", node.name);
                let registry = DEVICE_TREE_REGISTRY.read();
                for c in compatible.all() {
                    if let Some(f) = registry.get(c) {
                        f(&node);
                    }
                }
            }
        }
    }
}

/// 返回 DRAM 的起始物理地址与总大小（合并所有 memory.regions）
/// # 返回值
/// * `Option<(usize, usize)>` - 返回起始地址和大小的元组，如果没有有效的内存区域则返回 None
pub fn dram_info() -> Option<(usize, usize)> {
    let mut start = usize::MAX;
    let mut end = 0usize;

    for region in FDT.memory().regions() {
        let s = region.starting_address as usize;
        let size = region.size.unwrap_or(0) as usize;
        let e = s.saturating_add(size);
        if size == 0 {
            continue;
        }
        if s < start {
            start = s;
        }
        if e > end {
            end = e;
        }
    }

    if start < end {
        Some((start, end - start))
    } else {
        None
    }
}
