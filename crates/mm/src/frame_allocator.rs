//! 帧分配器模块
//!
//! 本模块提供物理内存帧的分配和跟踪功能。
//!
//! ## 分配策略（位图）
//!
//! 分配器使用位图（bitmap）跟踪每个物理帧的分配状态：
//!
//! - **bitmap**：每个 bit 表示一个物理帧（0=空闲，1=已分配）
//! - **last_alloc_hint**：上次分配位置提示，利用局部性加速查找
//!
//! 分配流程：
//!
//! 1. 单帧分配：从 last_alloc_hint 开始循环查找第一个空闲位
//! 2. 连续帧分配：扫描位图查找连续空闲区域（利用 u64 边界优化）
//! 3. 对齐分配：从对齐边界开始查找满足要求的连续空闲帧
//!
//! 释放时直接清除对应 bit，O(1) 操作，无需维护回收栈。
//!
//! ## RAII：自动回收
//!
//! - [`FrameTracker`]：单帧 RAII 包装器，`Drop` 时自动回收
//! - [`FrameRangeTracker`]：连续帧范围 RAII 包装器，`Drop` 时自动回收
//!
//! 这使得"分配后忘记释放"的错误更难发生，也便于在异常路径上保持资源正确回收。
//!
//! ## 对齐连续帧分配
//!
//! [`alloc_contig_frames_aligned`] 支持按"页数"对齐起始地址（例如按 2MB 对齐，
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
/// 采用位图策略跟踪每个物理帧的分配状态。
pub struct FrameAllocator {
    /// 物理帧的起始 Ppn。
    start: Ppn,
    /// 物理帧的结束 Ppn (不包含)。
    end: Ppn,
    /// 位图数据（每个 bit 表示一个帧：0=空闲，1=已分配）。
    /// 使用 Vec<u64> 存储，利用 64 位操作优化查找。
    bitmap: Vec<u64>,
    /// 总帧数。
    total_frames: usize,
    /// 已分配帧数（用于快速统计）。
    allocated_count: usize,
    /// 上次分配的位置提示（用于加速单帧分配）。
    last_alloc_hint: usize,
}

/// 位图帧分配器的实现
impl FrameAllocator {
    /// 创建一个新的帧分配器实例。
    pub fn new() -> Self {
        FrameAllocator {
            // 使用 usize::MAX 作为初始值，表示未初始化状态
            start: Ppn::from_usize(usize::MAX),
            end: Ppn::from_usize(usize::MAX),
            bitmap: Vec::new(), // 空 Vec，不分配内存
            total_frames: 0,
            allocated_count: 0,
            last_alloc_hint: 0,
        }
    }

    /// 初始化帧分配器，设置可用的物理内存范围。
    pub fn init(&mut self, start: Ppn, end: Ppn) {
        self.start = start;
        self.end = end;
        self.total_frames = end.as_usize() - start.as_usize();

        // 计算位图需要的 u64 数量
        let bitmap_u64_count = (self.total_frames + 63) / 64;

        // 分配位图（此时堆分配器已初始化）
        self.bitmap = alloc::vec![0u64; bitmap_u64_count];

        self.allocated_count = 0;
        self.last_alloc_hint = 0;
    }

    /// 检查帧是否空闲
    #[inline]
    fn is_free(&self, frame_idx: usize) -> bool {
        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0
    }

    /// 标记帧为已分配
    #[inline]
    fn mark_allocated(&mut self, frame_idx: usize) {
        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        self.bitmap[word_idx] |= 1u64 << bit_idx;
    }

    /// 标记帧为空闲
    #[inline]
    fn mark_free(&mut self, frame_idx: usize) {
        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        self.bitmap[word_idx] &= !(1u64 << bit_idx);
    }

    /// 分配一个物理帧。
    /// 从 last_alloc_hint 开始循环查找第一个空闲位。
    pub fn alloc_frame(&mut self) -> Option<FrameTracker> {
        let bitmap_len = self.bitmap.len();
        if bitmap_len == 0 {
            return None;
        }

        // 从上次分配位置开始查找（局部性优化）
        let start_idx = self.last_alloc_hint;

        // 循环查找：[hint, end) + [0, hint)
        for offset in 0..bitmap_len {
            let idx = (start_idx + offset) % bitmap_len;
            let word = self.bitmap[idx];

            // 快速跳过全满的 u64
            if word == u64::MAX {
                continue;
            }

            // 找到第一个空闲位（trailing_zeros 找最低位的 0）
            let bit_pos = (!word).trailing_zeros() as usize;
            if bit_pos < 64 {
                let frame_idx = idx * 64 + bit_pos;

                // 检查是否超出范围
                if frame_idx >= self.total_frames {
                    continue;
                }

                // 标记为已分配
                self.mark_allocated(frame_idx);
                self.allocated_count += 1;
                self.last_alloc_hint = idx;

                let ppn = self.start + frame_idx;
                return Some(FrameTracker::new(ppn));
            }
        }

        None // 内存耗尽
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
        if num == 0 || num > self.total_frames - self.allocated_count {
            return None;
        }

        let mut consecutive = 0;
        let mut start_frame = 0;

        // 逐 u64 扫描查找连续空闲帧
        for idx in 0..self.bitmap.len() {
            let word = self.bitmap[idx];

            // 快速跳过全满的 u64
            if word == u64::MAX {
                consecutive = 0;
                continue;
            }

            // 如果整个 u64 都是空闲，快速累加
            if word == 0 {
                if consecutive == 0 {
                    start_frame = idx * 64;
                }
                let frames_in_word = 64.min(self.total_frames - idx * 64);
                consecutive += frames_in_word;

                if consecutive >= num {
                    // 找到足够的连续帧，标记为已分配
                    for i in 0..num {
                        self.mark_allocated(start_frame + i);
                    }
                    self.allocated_count += num;

                    let start_ppn = self.start + start_frame;
                    let range = PpnRange::from_start_len(start_ppn, num);
                    return Some(FrameRangeTracker::new(range));
                }
                continue;
            }

            // 逐位检查
            for bit in 0..64 {
                let frame_idx = idx * 64 + bit;
                if frame_idx >= self.total_frames {
                    break;
                }

                if (word & (1u64 << bit)) == 0 {
                    if consecutive == 0 {
                        start_frame = frame_idx;
                    }
                    consecutive += 1;

                    if consecutive == num {
                        // 找到足够的连续帧，标记为已分配
                        for i in 0..num {
                            self.mark_allocated(start_frame + i);
                        }
                        self.allocated_count += num;

                        let start_ppn = self.start + start_frame;
                        let range = PpnRange::from_start_len(start_ppn, num);
                        return Some(FrameRangeTracker::new(range));
                    }
                } else {
                    consecutive = 0;
                }
            }
        }

        None
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

        // 从对齐边界开始查找
        let mut frame_idx = 0;
        while frame_idx < self.total_frames {
            // 对齐到下一个边界
            let aligned_idx = (frame_idx + align_pages - 1) & !(align_pages - 1);
            if aligned_idx + num > self.total_frames {
                break;
            }

            // 检查从 aligned_idx 开始的 num 个帧是否都空闲
            let mut all_free = true;
            for i in 0..num {
                if !self.is_free(aligned_idx + i) {
                    all_free = false;
                    frame_idx = aligned_idx + i + 1;
                    break;
                }
            }

            if all_free {
                // 分配这些帧
                for i in 0..num {
                    self.mark_allocated(aligned_idx + i);
                }
                self.allocated_count += num;

                let start_ppn = self.start + aligned_idx;
                let range = PpnRange::from_start_len(start_ppn, num);
                return Some(FrameRangeTracker::new(range));
            }
        }

        None
    }

    /// 回收一个物理帧。
    fn dealloc_frame(&mut self, frame: &FrameTracker) {
        // 检查帧是否在有效范围内
        debug_assert!(
            frame.ppn() >= self.start && frame.ppn() < self.end,
            "dealloc_frame: frame out of range" // 回收帧超出范围
        );

        let ppn = frame.ppn();
        let frame_idx = ppn.as_usize() - self.start.as_usize();

        // 检查帧是否已被分配
        debug_assert!(
            !self.is_free(frame_idx),
            "dealloc_frame: double free detected" // 检测到重复释放
        );

        // 标记为空闲
        self.mark_free(frame_idx);
        self.allocated_count -= 1;
    }

    /// 回收一个连续的物理帧范围。
    fn dealloc_contig_frames(&mut self, frame_range: &FrameRangeTracker) {
        let start = frame_range.start_ppn();
        let end = frame_range.end_ppn();
        // 检查范围是否在有效范围内
        debug_assert!(
            start >= self.start && end <= self.end,
            "dealloc_contig_frames: frame range out of range" // 回收帧范围超出范围
        );

        let start_idx = start.as_usize() - self.start.as_usize();
        let len = frame_range.len();

        // 批量标记为空闲
        for i in 0..len {
            debug_assert!(
                !self.is_free(start_idx + i),
                "dealloc_contig_frames: double free detected" // 检测到重复释放
            );
            self.mark_free(start_idx + i);
        }
        self.allocated_count -= len;
    }

    /// 获取总的物理帧数
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// 获取已分配的帧数
    pub fn allocated_frames(&self) -> usize {
        self.allocated_count
    }

    /// 获取空闲的帧数
    pub fn free_frames(&self) -> usize {
        self.total_frames - self.allocated_count
    }

    /// 获取帧分配器的当前状态
    /// # 返回值
    /// - 总帧数
    /// - 已分配的帧数
    /// - 空闲的帧数
    pub fn get_stats(&self) -> (usize, usize, usize) {
        (
            self.total_frames,
            self.allocated_count,
            self.total_frames - self.allocated_count,
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
/// - 总帧数
/// - 已分配的帧数
/// - 空闲的帧数
pub fn get_stats() -> (usize, usize, usize) {
    FRAME_ALLOCATOR.lock().get_stats()
}
