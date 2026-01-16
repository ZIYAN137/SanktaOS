//! 帧分配器模块
//!
//! 本模块提供物理内存帧的分配和跟踪功能。
//!
//! ## 分配策略（水位线 + 回收栈）
//!
//! 分配器维护一段可分配的页号区间 `[start, end)`，并使用：
//!
//! - `cur`：水位线指针，表示“尚未被水位线分配过”的起始位置
//! - `recycled`：回收栈，保存已经释放的页号（会排序以便合并）
//!
//! 分配优先级：
//!
//! 1. 优先从 `recycled` 中取出可用页（重用已释放页，减少水位线推进）
//! 2. 若回收栈为空，则从 `[cur, end)` 顺序分配并推进水位线
//!
//! 回收时会尝试与水位线前的连续空闲区域合并（将 `cur` 回退），减少碎片。
//!
//! ## RAII：自动回收
//!
//! - [`FrameTracker`]：单帧 RAII 包装器，`Drop` 时自动回收
//! - [`FrameRangeTracker`]：连续帧范围 RAII 包装器，`Drop` 时自动回收
//!
//! 这使得“分配后忘记释放”的错误更难发生，也便于在异常路径上保持资源正确回收。
//!
//! ## 对齐连续帧分配
//!
//! [`alloc_contig_frames_aligned`] 支持按“页数”对齐起始地址（例如按 2MB 对齐，
//! 传入 `align_pages = 512`）。
//!
//! # 模块组成
//!
//! - [`FrameTracker`]：用于单个已分配帧的 **RAII** 封装器。
//! - [`FrameRangeTracker`]：用于已分配帧范围的 **RAII** 封装器。
//! - [`init_frame_allocator`]：初始化全局帧分配器。
//! - [`alloc_frame`]：分配单个帧。
//! - `alloc_frames`：分配多个（非连续）帧。
//! - `alloc_contig_frames`：分配多个连续帧。
//! - `alloc_contig_frames_aligned`：分配带对齐要求的多个连续帧。

use crate::address::{ConvertablePaddr, Paddr, PageNum, Ppn, PpnRange, UsizeConvert};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use sync::SpinLock;

// ============================================================================
// FrameTracker - 单帧 RAII 封装
// ============================================================================

/// 物理帧跟踪器。
/// 实现了 RAII 模式：当此结构体被 drop 时，它所管理的物理页帧会被自动回收。
#[derive(Debug)]
pub struct FrameTracker(Ppn);

impl FrameTracker {
    /// 创建一个新的 FrameTracker。
    /// 在创建时，会自动将该物理页帧清零。
    pub fn new(ppn: Ppn) -> Self {
        clear_frame(ppn);
        FrameTracker(ppn)
    }

    /// 获取此帧跟踪器所管理的物理页号 (Ppn)。
    pub fn ppn(&self) -> Ppn {
        self.0
    }
}

impl Drop for FrameTracker {
    /// 自动回收物理页帧。
    fn drop(&mut self) {
        dealloc_frame(self);
    }
}

// ============================================================================
// FrameRangeTracker - 连续帧范围 RAII 封装
// ============================================================================

/// 连续物理帧范围跟踪器。
/// 实现了 RAII 模式：当此结构体被 drop 时，它所管理的物理页帧范围会被自动回收。
#[derive(Debug)]
pub struct FrameRangeTracker {
    range: PpnRange,
}

impl FrameRangeTracker {
    /// 创建一个新的 FrameRangeTracker。
    /// 在创建时，会自动将该范围内的所有物理页帧清零。
    pub fn new(range: PpnRange) -> Self {
        for ppn in range {
            clear_frame(ppn);
        }
        FrameRangeTracker { range }
    }

    /// 获取连续帧范围的起始物理页号 (Ppn)。
    pub fn start_ppn(&self) -> Ppn {
        self.range.start()
    }

    /// 获取连续帧范围的结束物理页号 (Ppn)（不包含）。
    pub fn end_ppn(&self) -> Ppn {
        self.range.end()
    }

    /// 获取连续帧范围内的帧数量。
    pub fn len(&self) -> usize {
        self.range.len()
    }

    /// 获取连续帧范围的引用。
    pub fn range(&self) -> &PpnRange {
        &self.range
    }
}

impl Drop for FrameRangeTracker {
    /// 自动回收连续物理页帧。
    fn drop(&mut self) {
        dealloc_contig_frames(self);
    }
}

// ============================================================================
// TrackedFrames - 帧集合枚举
// ============================================================================

/// 跟踪的物理帧集合。
/// 用于封装单个、多个不连续或多个连续的物理帧。
#[derive(Debug)]
pub enum TrackedFrames {
    /// 单个物理帧。
    Single(FrameTracker),
    /// 多个不连续物理帧。
    Multiple(Vec<FrameTracker>),
    /// 多个连续物理帧。
    Contiguous(FrameRangeTracker),
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 将指定的物理页帧清零。
fn clear_frame(ppn: Ppn) {
    let page_size = crate::mm_config().page_size();
    unsafe {
        // 将 Ppn 转换为虚拟地址指针
        let va = ppn.start_addr().to_vaddr().as_mut_ptr::<u8>();
        // 写入 PAGE_SIZE 字节的 0
        core::ptr::write_bytes(va, 0, page_size);
    }
}

// ============================================================================
// 全局帧分配器
// ============================================================================

lazy_static! {
    /// 全局物理帧分配器，由自旋锁保护。
    static ref FRAME_ALLOCATOR: SpinLock<FrameAllocator> = SpinLock::new(FrameAllocator::new());
}

/// 物理帧分配器。
/// 采用简单的"延迟分配"策略，并使用回收栈来重用已释放的帧。
pub struct FrameAllocator {
    /// 物理帧的起始 Ppn。
    start: Ppn,
    /// 物理帧的结束 Ppn (不包含)。
    end: Ppn,
    /// 下一个要分配的物理帧 Ppn（用于连续分配区域）。
    cur: Ppn,
    /// 回收的物理帧堆栈。
    recycled: Vec<Ppn>,
}

/// 延迟分配 (lazy frame allocator) 的实现
impl FrameAllocator {
    /// 创建一个新的帧分配器实例。
    pub fn new() -> Self {
        FrameAllocator {
            // 使用 usize::MAX 作为初始值，表示未初始化状态
            start: Ppn::from_usize(usize::MAX),
            end: Ppn::from_usize(usize::MAX),
            cur: Ppn::from_usize(usize::MAX),
            recycled: Vec::new(),
        }
    }

    /// 初始化帧分配器，设置可用的物理内存范围。
    pub fn init(&mut self, start: Ppn, end: Ppn) {
        self.start = start;
        self.end = end;
        self.cur = start;
    }

    /// 分配一个物理帧。
    /// 优先从回收栈中取出，否则从连续未分配区域分配。
    pub fn alloc_frame(&mut self) -> Option<FrameTracker> {
        if let Some(ppn) = self.recycled.pop() {
            // 从回收栈中分配
            Some(FrameTracker::new(ppn))
        } else if self.cur < self.end {
            // 从连续未分配区域分配
            let ppn = self.cur;
            self.cur.step(); // 移动当前分配指针
            Some(FrameTracker::new(ppn))
        } else {
            // 物理内存耗尽
            None
        }
    }

    /// 分配指定数量的物理帧（不保证连续）。
    pub fn alloc_frames(&mut self, num: usize) -> Option<Vec<FrameTracker>> {
        let mut frames = Vec::with_capacity(num);
        for _ in 0..num {
            if let Some(frame) = self.alloc_frame() {
                frames.push(frame);
            } else {
                // 分配失败，需要将已分配的帧回收
                // 由于 FrameTracker 实现了 Drop，这里直接 drop frames 即可
                return None;
            }
        }
        Some(frames)
    }

    /// 分配指定数量的**连续**物理帧。
    pub fn alloc_contig_frames(&mut self, num: usize) -> Option<FrameRangeTracker> {
        if num == 0 {
            return None;
        }

        // 检查是否有足够的连续帧
        let required_end = self.cur + num;
        if required_end <= self.end {
            let start = self.cur;
            // 移动分配指针到新的连续区域之后
            self.cur = required_end;
            let range = PpnRange::from_start_len(start, num);
            Some(FrameRangeTracker::new(range))
        } else {
            // 物理内存不足
            None
        }
    }

    /// 分配指定数量的**连续**物理帧，并确保起始地址对齐到 `align_pages` 页的边界。
    pub fn alloc_contig_frames_aligned(
        &mut self,
        num: usize,
        align_pages: usize,
    ) -> Option<FrameRangeTracker> {
        if num == 0 {
            return None;
        }

        debug_assert!(
            align_pages.is_power_of_two(),
            "Alignment must be power of 2" // 对齐必须是 2 的幂
        );

        // 向上对齐当前分配指针 `self.cur`
        let aligned_cur_val =
            (self.cur.as_usize() + align_pages - 1).div_ceil(align_pages) * align_pages;
        let aligned_cur = Ppn::from_usize(aligned_cur_val);

        // 检查对齐后是否有足够的空间
        let required_end = aligned_cur + num;
        if required_end <= self.end {
            // 将跳过的帧（self.cur 到 aligned_cur 之间）加入 recycled 栈
            for ppn_val in self.cur.as_usize()..aligned_cur.as_usize() {
                self.recycled.push(Ppn::from_usize(ppn_val));
            }

            // 更新当前分配指针
            self.cur = required_end;
            let range = PpnRange::from_start_len(aligned_cur, num);
            Some(FrameRangeTracker::new(range))
        } else {
            // 物理内存不足
            None
        }
    }

    /// 回收一个物理帧。
    /// 尝试将回收的帧与当前分配指针前的连续空闲区域合并。
    fn dealloc_frame(&mut self, frame: &FrameTracker) {
        // 检查帧是否在有效范围内
        debug_assert!(
            frame.ppn() >= self.start && frame.ppn() < self.end,
            "dealloc_frame: frame out of range" // 回收帧超出范围
        );
        // 检查帧是否已被分配 (即在当前指针之前且不在回收栈中)
        debug_assert!(
            frame.ppn() < self.cur && self.recycled.iter().all(|&ppn| ppn != frame.ppn()),
        );

        let ppn = frame.ppn();
        self.recycled.push(ppn);
        // 对回收栈进行排序，以便于连续合并检查
        self.recycled.sort_unstable();

        if let Some(&last) = self.recycled.last() {
            // 检查回收栈顶部的帧是否是当前分配指针前面的连续帧
            if last + 1 == self.cur {
                // 回收连续帧
                let mut new_cur = last;
                self.recycled.pop();
                while let Some(&top) = self.recycled.last() {
                    if top + 1 == new_cur {
                        new_cur = top;
                        self.recycled.pop();
                    } else {
                        break;
                    }
                }
                self.cur = new_cur;
            }
        }
    }

    /// 回收一个连续的物理帧范围。
    /// 尝试将回收的帧与当前分配指针前的连续空闲区域合并。
    fn dealloc_contig_frames(&mut self, frame_range: &FrameRangeTracker) {
        let start = frame_range.start_ppn();
        let end = frame_range.end_ppn();
        // 检查范围是否在有效范围内
        debug_assert!(
            start >= self.start && end <= self.end,
            "dealloc_contig_frames: frame range out of range" // 回收帧范围超出范围
        );
        // 检查范围是否已被分配 (即在当前指针之前)
        debug_assert!(
            end <= self.cur,
            "dealloc_contig_frames: frame range not allocated" // 回收帧范围未被分配
        );

        // 将连续帧范围内的所有 Ppn 加入回收栈
        for ppn in frame_range.range().into_iter() {
            self.recycled.push(ppn);
        }
        // 排序以支持连续合并
        self.recycled.sort_unstable();

        if let Some(&last) = self.recycled.last() {
            // 检查回收栈顶部的帧是否是当前分配指针前面的连续帧
            if last + 1 == self.cur {
                // 回收连续帧
                let mut new_cur = last;
                self.recycled.pop();
                while let Some(&top) = self.recycled.last() {
                    if top + 1 == new_cur {
                        new_cur = top;
                        self.recycled.pop();
                    } else {
                        break;
                    }
                }
                self.cur = new_cur;
            }
        }
    }

    /// 获取总的物理帧数
    pub fn total_frames(&self) -> usize {
        self.end.as_usize() - self.start.as_usize()
    }

    /// 获取已分配的帧数
    pub fn allocated_frames(&self) -> usize {
        let allocated = self.cur.as_usize() - self.start.as_usize();
        let recycled = self.recycled.len();
        allocated - recycled
    }

    /// 获取空闲的帧数
    pub fn free_frames(&self) -> usize {
        let total = self.total_frames();
        let allocated = self.allocated_frames();
        total - allocated
    }

    /// 获取帧分配器的当前状态
    /// # 返回值
    /// - 当前分配指针的 Ppn
    /// - 物理帧的结束 Ppn (不包含)
    /// - 回收栈的长度
    /// - 已分配的帧数
    /// - 空闲的帧数
    pub fn get_stats(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.cur.as_usize(),
            self.end.as_usize(),
            self.recycled.len(),
            self.allocated_frames(),
            self.free_frames(),
        )
    }
}

impl Default for FrameAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 公共 API
// ============================================================================

/// 使用可用的物理内存范围初始化全局帧分配器。
///
/// # 参数
///
/// * `start_addr` - 可用物理内存的起始地址
/// * `end_addr` - 可用物理内存的结束地址
pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    // 将起始地址向上取整到页号
    let start_ppn = Ppn::from_addr_ceil(Paddr::from_usize(start_addr));
    // 将结束地址向下取整到页号
    let end_ppn = Ppn::from_addr_floor(Paddr::from_usize(end_addr));

    let mut allocator = FRAME_ALLOCATOR.lock();
    allocator.init(start_ppn, end_ppn);
}

/// 分配一个物理帧。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameTracker)`；否则返回 `None`。
pub fn alloc_frame() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc_frame()
}

/// 分配多个物理帧（不保证连续）。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(Vec<FrameTracker>)`；否则返回 `None`。
pub fn alloc_frames(num: usize) -> Option<Vec<FrameTracker>> {
    FRAME_ALLOCATOR.lock().alloc_frames(num)
}

/// 分配指定数量的**连续**物理帧。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameRangeTracker)`；否则返回 `None`。
pub fn alloc_contig_frames(num: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR.lock().alloc_contig_frames(num)
}

/// 分配指定数量的**连续**物理帧，并确保起始地址对齐。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
/// * `align_pages` - 对齐的页数（必须是 2 的幂）。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameRangeTracker)`；否则返回 `None`。
pub fn alloc_contig_frames_aligned(num: usize, align_pages: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR
        .lock()
        .alloc_contig_frames_aligned(num, align_pages)
}

/// 回收一个物理帧。此函数由 FrameTracker 的 Drop 实现调用。
fn dealloc_frame(frame: &FrameTracker) {
    FRAME_ALLOCATOR.lock().dealloc_frame(frame);
}

/// 回收多个物理帧（不保证连续）。
#[allow(dead_code)]
fn dealloc_frames(frames: &[FrameTracker]) {
    let mut allocator = FRAME_ALLOCATOR.lock();
    for frame in frames {
        allocator.dealloc_frame(frame);
    }
}

/// 回收一个连续的物理帧范围。此函数由 FrameRangeTracker 的 Drop 实现调用。
fn dealloc_contig_frames(frame_range: &FrameRangeTracker) {
    FRAME_ALLOCATOR.lock().dealloc_contig_frames(frame_range);
}

/// 获取总的物理帧数
pub fn get_total_frames() -> usize {
    FRAME_ALLOCATOR.lock().total_frames()
}

/// 获取已分配的帧数
pub fn get_allocated_frames() -> usize {
    FRAME_ALLOCATOR.lock().allocated_frames()
}

/// 获取空闲的帧数
pub fn get_free_frames() -> usize {
    FRAME_ALLOCATOR.lock().free_frames()
}

/// 获取帧分配器的当前状态
///
/// # 返回值
/// - 当前分配指针的 Ppn
/// - 物理帧的结束 Ppn (不包含)
/// - 回收栈的长度
/// - 已分配的帧数
/// - 空闲的帧数
pub fn get_stats() -> (usize, usize, usize, usize, usize) {
    FRAME_ALLOCATOR.lock().get_stats()
}
