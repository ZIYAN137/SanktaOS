//! 页码抽象模块
//!
//! 此模块定义了表示页码 (Page Number) 的 Trait 和具体的页码类型 (Ppn, Vpn)，
//! 以及用于处理连续页码的范围结构 (PageNumRange)。
//!
//! 页码是地址空间中页 (Page) 的索引，它将内存管理抽象与底层硬件地址解耦。

use crate::address::operations::{AlignOps, CalcOps, UsizeConvert};
use crate::address::types::{Address, Paddr, Vaddr};
use core::ops::Range;

/// 获取页大小
#[inline]
fn page_size() -> usize {
    crate::mm_config().page_size()
}

/// [PageNum] Trait
/// ---------------------
/// 表示一个页码的 Trait。所有页码类型 (如 Ppn 和 Vpn) 必须实现此 Trait。
///
/// 它依赖于 `CalcOps` (算术和位操作) 和 `UsizeConvert` (与 usize 转换)。
pub trait PageNum:
    CalcOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    /// 此页码类型关联的地址类型（例如 Ppn 关联 Paddr，Vpn 关联 Vaddr）。
    type TAddress: Address + AlignOps; // PageNum 的地址需要支持 AlignOps

    /// 将页码增加 1。
    fn step(&mut self) {
        self.step_by(1);
    }

    /// 将页码增加给定的偏移量 (页数)。
    ///
    /// # 参数
    /// * `offset`: 要增加的页数。
    fn step_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() + offset);
    }

    /// 将页码减少 1。
    fn step_back(&mut self) {
        self.step_back_by(1);
    }

    /// 将页码减少给定的偏移量 (页数)。
    ///
    /// # 参数
    /// * `offset`: 要减少的页数。
    fn step_back_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() - offset);
    }

    /// 将地址转换为页码 (向下取整，即页的起始页码)。
    ///
    /// # 参数
    /// * `addr`: 要转换的地址。
    ///
    /// # 返回
    /// 包含该地址的页的页码。
    fn from_addr_floor(addr: Self::TAddress) -> Self {
        // 先向下对齐到页边界，再除以页大小 PAGE_SIZE
        Self::from_usize(addr.align_down_to_page().as_usize() / page_size())
    }

    /// 将地址转换为页码 (向上取整，即如果地址未对齐，则指向下一个页码)。
    ///
    /// # 参数
    /// * `addr`: 要转换的地址。
    ///
    /// # 返回
    /// 包含该地址的页码。如果地址位于页内，则返回该页页码；如果地址是页的起始，则返回该页页码；
    /// 如果地址是页的结束（例如 0x1000），则返回下一页的页码（例如 1）。
    fn from_addr_ceil(addr: Self::TAddress) -> Self {
        // 先向上对齐到页边界，再除以页大小 PAGE_SIZE
        Self::from_usize(addr.align_up_to_page().as_usize() / page_size())
    }

    /// 获取该页码对应的起始地址。
    ///
    /// # 返回
    /// 页的起始地址。
    fn start_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize(self.as_usize() * page_size())
    }

    /// 获取该页码对应的结束地址 (即下一页的起始地址)。
    ///
    /// # 返回
    /// 页的结束地址 (不包含在页内)。
    fn end_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize((self.as_usize() + 1) * page_size())
    }

    /// 计算两个页码之间的页数差。
    ///
    /// # 参数
    /// * `other`: 另一个页码。
    ///
    /// # 返回
    /// 两个页码之间的带符号整数差值。
    fn diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }
}

/// `impl_page_num!` 宏
/// ---------------------
/// 快速为给定类型实现 `UsizeConvert` 和 `PageNum` Trait。
///
/// 此宏同时调用 `impl_calc_ops!` 来实现所有的算术和位操作。
///
/// # 使用示例
/// ```ignore
/// #[repr(transparent)]
/// #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
/// pub struct MyPpn(pub usize);
/// impl_page_num!(MyPpn, Paddr); // Paddr 是关联的地址类型
/// ```
#[macro_export]
macro_rules! impl_page_num {
    ($type:ty, $addr_type:ty) => {
        // 1. 实现 UsizeConvert，允许与 usize 互相转换
        impl $crate::address::operations::UsizeConvert for $type {
            fn as_usize(&self) -> usize {
                self.0
            }

            fn from_usize(value: usize) -> Self {
                Self(value)
            }
        }

        // 2. 自动实现 CalcOps (算术和位运算)
        $crate::impl_calc_ops!($type);

        // 3. 实现 PageNum Trait，绑定地址类型
        impl $crate::address::page_num::PageNum for $type {
            type TAddress = $addr_type;
        }
    };
}

/// [Ppn] (Physical Page Number)
/// ---------------------
/// 物理页码，对应物理地址 (Paddr)。
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ppn(pub usize);
impl_page_num!(Ppn, Paddr);

/// [Vpn] (Virtual Page Number)
/// ---------------------
/// 虚拟页码，对应虚拟地址 (Vaddr)。
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Vpn(pub usize);
impl_page_num!(Vpn, Vaddr);

/// [PageNumRange]
/// ---------------------
/// 泛型页码范围结构，表示一个半开半闭的区间 `[start, end)`。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageNumRange<T>
where
    T: PageNum,
{
    /// 范围的起始页码 (包含)。
    pub start: T,
    /// 范围的结束页码 (不包含)。
    pub end: T,
}

impl<T> PageNumRange<T>
where
    T: PageNum,
{
    /// 创建一个新的页码范围。
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }

    /// 从 Rust 标准库的 `Range<T>` 创建一个页码范围。
    pub fn from_range(range: Range<T>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    /// 从起始页码和长度 (页数) 创建一个页码范围。
    pub fn from_start_len(start: T, len: usize) -> Self {
        Self {
            start,
            end: T::from_usize(start.as_usize() + len),
        }
    }

    /// 获取起始页码。
    pub fn start(&self) -> T {
        self.start
    }

    /// 获取结束页码 (不包含)。
    pub fn end(&self) -> T {
        self.end
    }

    /// 获取范围内的页数。
    pub fn len(&self) -> usize {
        debug_assert!(self.end.as_usize() >= self.start.as_usize());
        self.end.as_usize() - self.start.as_usize()
    }

    /// 检查范围是否为空 (即 start == end)。
    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    /// 检查范围是否包含给定的页码。
    pub fn contains(&self, addr: T) -> bool {
        addr >= self.start && addr < self.end
    }

    /// 检查范围是否包含另一个范围。
    pub fn contains_range(&self, other: &Self) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    /// 检查此范围是否包含在另一个范围中。
    pub fn contains_in(&self, other: &Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }

    /// 检查两个范围是否重叠。
    ///
    /// 注意: PageNumRange 是 [start, end)，相邻的范围不视为重叠。
    pub fn overlaps(&self, other: &Self) -> bool {
        !(self.end <= other.start || self.start >= other.end)
    }

    /// 获取范围的迭代器。
    pub fn iter(&self) -> PageNumRangeIterator<T> {
        PageNumRangeIterator {
            range: *self,
            current: self.start,
        }
    }
}

impl<T> IntoIterator for PageNumRange<T>
where
    T: PageNum,
{
    type Item = T;
    type IntoIter = PageNumRangeIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [PageNumRangeIterator]
/// ---------------------
/// 页码范围的迭代器，按升序返回范围内的每个页码。
pub struct PageNumRangeIterator<T>
where
    T: PageNum,
{
    range: PageNumRange<T>,
    current: T,
}

impl<T> Iterator for PageNumRangeIterator<T>
where
    T: PageNum,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.range.end {
            return None;
        }
        let result = self.current;
        self.current.step(); // 步进到下一页
        Some(result)
    }
}

/// 物理页码范围的类型别名
pub type PpnRange = PageNumRange<Ppn>;
/// 虚拟页码范围的类型别名
pub type VpnRange = PageNumRange<Vpn>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_num_start_end_addr() {
        let vpn = Vpn::from_usize(1);
        assert_eq!(vpn.start_addr().as_usize(), 4096);
        assert_eq!(vpn.end_addr().as_usize(), 8192);
    }

    #[test]
    fn test_page_num_from_addr_floor_ceil() {
        let a = Vaddr::from_usize(4096);
        assert_eq!(Vpn::from_addr_floor(a).as_usize(), 1);
        assert_eq!(Vpn::from_addr_ceil(a).as_usize(), 1);

        let b = Vaddr::from_usize(4097);
        assert_eq!(Vpn::from_addr_floor(b).as_usize(), 1);
        assert_eq!(Vpn::from_addr_ceil(b).as_usize(), 2);
    }
}
