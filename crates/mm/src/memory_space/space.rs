//! 内存空间核心实现

use core::cmp::Ordering;
use core::marker::PhantomData;

use crate::address::{PageNum, Ppn, UsizeConvert, Vaddr, Vpn, VpnRange};
use crate::arch_ops::arch_ops;
use crate::memory_space::MmapFile;
use crate::memory_space::mapping_area::{AreaType, MapType, MappingArea};
use crate::mm_config;
use crate::page_table::{PageTableEntry, PageTableInner, PagingError, UniversalPTEFlag};
use alloc::vec::Vec;

/// 表示地址空间的内存空间结构体
#[derive(Debug)]
pub struct MemorySpace<PT: PageTableInner<E>, E: PageTableEntry> {
    /// 与此内存空间关联的页表
    page_table: PT,

    /// 此内存空间中的映射区域列表
    areas: Vec<MappingArea>,

    /// 堆的起始地址 (brk 系统调用使用，仅限用户空间)
    heap_start: Option<Vpn>,

    /// PhantomData for E
    _marker: PhantomData<E>,
}

impl<PT: PageTableInner<E>, E: PageTableEntry> MemorySpace<PT, E> {
    /// 创建一个新的空内存空间
    pub fn new() -> Self {
        MemorySpace {
            page_table: PT::new(),
            areas: Vec::new(),
            heap_start: None,
            _marker: PhantomData,
        }
    }

    /// 返回页表的引用
    pub fn page_table(&self) -> &PT {
        &self.page_table
    }

    /// 返回页表的可变引用
    pub fn page_table_mut(&mut self) -> &mut PT {
        &mut self.page_table
    }

    /// 返回根页表的物理页号 (PPN)
    pub fn root_ppn(&self) -> Ppn {
        self.page_table.root_ppn()
    }

    /// 返回所有映射区域的引用
    pub fn areas(&self) -> &Vec<MappingArea> {
        &self.areas
    }

    /// 返回所有映射区域的可变引用
    pub fn areas_mut(&mut self) -> &mut Vec<MappingArea> {
        &mut self.areas
    }

    /// 获取当前的 brk 值（堆的当前结束地址）
    pub fn current_brk(&self) -> Option<usize> {
        self.areas
            .iter()
            .find(|a| a.area_type() == AreaType::UserHeap)
            .map(|a| a.vpn_range().end().start_addr().as_usize())
            .or_else(|| self.heap_start.map(|vpn| vpn.start_addr().as_usize()))
    }

    /// 设置用户堆的起始地址（brk 的下界）
    pub fn set_heap_start(&mut self, heap_start: Vpn) {
        self.heap_start = Some(heap_start);
    }

    /// 获取堆的起始地址
    pub fn heap_start(&self) -> Option<Vpn> {
        self.heap_start
    }

    /// 从当前地址空间中向指定虚拟地址写入字节序列（跨页安全）
    pub fn write_bytes_at(&mut self, va: usize, bytes: &[u8]) -> Result<(), PagingError> {
        if bytes.is_empty() {
            return Ok(());
        }

        let page_size = mm_config().page_size();
        let mut written = 0usize;
        while written < bytes.len() {
            let cur_va = va.checked_add(written).ok_or(PagingError::InvalidAddress)?;
            let paddr = self
                .page_table
                .translate(Vaddr::from_usize(cur_va))
                .ok_or(PagingError::InvalidAddress)?;
            let paddr_usize = paddr.as_usize();
            let page_base = paddr_usize & !(page_size - 1);
            let page_off = paddr_usize & (page_size - 1);

            let take = core::cmp::min(bytes.len() - written, page_size - page_off);
            let dst = (arch_ops().paddr_to_vaddr(page_base) + page_off) as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(bytes[written..].as_ptr(), dst, take);
            }
            written += take;
        }

        Ok(())
    }

    /// 从指定虚拟地址读取字节序列（跨页安全）
    pub fn read_bytes_at(&self, va: usize, out: &mut [u8]) -> Result<(), PagingError> {
        if out.is_empty() {
            return Ok(());
        }

        let page_size = mm_config().page_size();
        let mut read = 0usize;
        while read < out.len() {
            let cur_va = va.checked_add(read).ok_or(PagingError::InvalidAddress)?;
            let paddr = self
                .page_table
                .translate(Vaddr::from_usize(cur_va))
                .ok_or(PagingError::InvalidAddress)?;
            let paddr_usize = paddr.as_usize();
            let page_base = paddr_usize & !(page_size - 1);
            let page_off = paddr_usize & (page_size - 1);

            let take = core::cmp::min(out.len() - read, page_size - page_off);
            let src = (arch_ops().paddr_to_vaddr(page_base) + page_off) as *const u8;
            unsafe {
                core::ptr::copy_nonoverlapping(src, out[read..].as_mut_ptr(), take);
            }
            read += take;
        }
        Ok(())
    }

    /// 读取 u64
    pub fn read_u64_at(&self, va: usize) -> Result<u64, PagingError> {
        let mut buf = [0u8; 8];
        self.read_bytes_at(va, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    /// 读取 i64
    pub fn read_i64_at(&self, va: usize) -> Result<i64, PagingError> {
        let mut buf = [0u8; 8];
        self.read_bytes_at(va, &mut buf)?;
        Ok(i64::from_le_bytes(buf))
    }

    /// 写入 usize
    pub fn write_usize_at(&mut self, va: usize, value: usize) -> Result<(), PagingError> {
        self.write_bytes_at(va, &value.to_ne_bytes())
    }

    /// 插入一个新的映射区域并检测重叠
    pub fn insert_area(&mut self, mut area: MappingArea) -> Result<(), PagingError> {
        for existing in &self.areas {
            if existing.vpn_range().overlaps(&area.vpn_range()) {
                return Err(PagingError::AlreadyMapped);
            }
        }

        area.map(&mut self.page_table)?;
        self.areas.push(area);

        Ok(())
    }

    /// 插入一个帧映射区域，并可选择复制数据
    pub fn insert_framed_area(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        data: Option<&[u8]>,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags, file);
        self.insert_area(area)?;

        if let Some(data) = data {
            let area = self.areas.last_mut().unwrap();
            area.copy_data(&mut self.page_table, data, 0);
        }

        Ok(())
    }

    /// 插入一个"保留"区域（不建立页表映射）
    pub fn insert_reserved_area(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Reserved, flags, file);
        self.insert_area(area)?;
        Ok(())
    }

    /// 插入一个帧映射区域，并可选择复制数据（带偏移量）
    pub fn insert_framed_area_with_offset(
        &mut self,
        vpn_range: VpnRange,
        area_type: AreaType,
        flags: UniversalPTEFlag,
        data: Option<&[u8]>,
        offset: usize,
        file: Option<MmapFile>,
    ) -> Result<(), PagingError> {
        let area = MappingArea::new(vpn_range, area_type, MapType::Framed, flags, file);
        self.insert_area(area)?;

        if let Some(data) = data {
            let area = self.areas.last_mut().unwrap();
            area.copy_data(&mut self.page_table, data, offset);
        }

        Ok(())
    }

    /// 查找包含给定 VPN 的区域
    pub fn find_area(&self, vpn: Vpn) -> Option<&MappingArea> {
        self.areas
            .iter()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// 查找包含给定 VPN 的区域（可变）
    pub fn find_area_mut(&mut self, vpn: Vpn) -> Option<&mut MappingArea> {
        self.areas
            .iter_mut()
            .find(|area| area.vpn_range().contains(vpn))
    }

    /// 通过 VPN 移除并取消映射一个区域
    pub fn remove_area(&mut self, vpn: Vpn) -> Result<(), PagingError> {
        if let Some(pos) = self.areas.iter().position(|a| a.vpn_range().contains(vpn)) {
            let mut area = self.areas.remove(pos);
            area.unmap(&mut self.page_table)?;
            Ok(())
        } else {
            Err(PagingError::NotMapped)
        }
    }

    /// 通过 VPN 范围移除并取消映射区域
    pub fn remove_area_by_range(&mut self, vpn_range: VpnRange) -> Result<(), PagingError> {
        if let Some(pos) = self.areas.iter().position(|a| a.vpn_range() == vpn_range) {
            let mut area = self.areas.remove(pos);
            area.unmap(&mut self.page_table)?;
            Ok(())
        } else {
            Err(PagingError::NotMapped)
        }
    }

    /// 查找与给定范围重叠的所有区域的索引
    pub fn find_overlapping_areas(&self, vpn_range: &VpnRange) -> Vec<usize> {
        self.areas
            .iter()
            .enumerate()
            .filter(|(_, area)| area.vpn_range().overlaps(vpn_range))
            .map(|(i, _)| i)
            .collect()
    }

    /// 在指定范围内查找空闲的虚拟地址空间
    pub fn find_free_area(&self, start: Vpn, end: Vpn, size_pages: usize) -> Option<VpnRange> {
        let mut current = start;

        while current.as_usize() + size_pages <= end.as_usize() {
            let candidate =
                VpnRange::new(current, Vpn::from_usize(current.as_usize() + size_pages));

            let overlaps = self
                .areas
                .iter()
                .any(|area| area.vpn_range().overlaps(&candidate));

            if !overlaps {
                return Some(candidate);
            }

            // 找到下一个可能的起始位置
            let next_start = self
                .areas
                .iter()
                .filter(|area| area.vpn_range().overlaps(&candidate))
                .map(|area| area.vpn_range().end())
                .max();

            match next_start {
                Some(next) => current = next,
                None => break,
            }
        }

        None
    }

    /// 克隆内存空间（用于 fork）
    pub fn clone_for_fork(&self) -> Result<Self, PagingError> {
        let mut new_space = Self::new();

        for area in &self.areas {
            let is_kernel = matches!(
                area.area_type(),
                AreaType::KernelText
                    | AreaType::KernelRodata
                    | AreaType::KernelData
                    | AreaType::KernelBss
                    | AreaType::KernelStack
                    | AreaType::KernelHeap
                    | AreaType::KernelMmio
            );

            if is_kernel {
                // 内核区域：只克隆元数据并重新映射
                let mut new_area = area.clone_metadata();
                new_area.map(&mut new_space.page_table)?;
                new_space.areas.push(new_area);
            } else {
                // 用户区域：克隆数据
                let new_area = area.clone_with_data(&mut new_space.page_table)?;
                new_space.areas.push(new_area);
            }
        }

        new_space.heap_start = self.heap_start;

        Ok(new_space)
    }

    /// 扩展堆
    pub fn extend_heap(&mut self, new_end: Vpn) -> Result<(), PagingError> {
        let heap_area = self
            .areas
            .iter_mut()
            .find(|a| a.area_type() == AreaType::UserHeap);

        match heap_area {
            Some(area) => {
                let current_end = area.vpn_range().end();
                match new_end.as_usize().cmp(&current_end.as_usize()) {
                    Ordering::Greater => {
                        let count = new_end.as_usize() - current_end.as_usize();
                        area.extend(&mut self.page_table, count)?;
                    }
                    Ordering::Less => {
                        let count = current_end.as_usize() - new_end.as_usize();
                        area.shrink(&mut self.page_table, count)?;
                    }
                    Ordering::Equal => {}
                }
                Ok(())
            }
            None => {
                // 创建新的堆区域
                let heap_start = self.heap_start.ok_or(PagingError::InvalidAddress)?;
                if new_end.as_usize() < heap_start.as_usize() {
                    return Err(PagingError::InvalidAddress);
                }
                let vpn_range = VpnRange::new(heap_start, new_end);
                self.insert_framed_area(
                    vpn_range,
                    AreaType::UserHeap,
                    UniversalPTEFlag::user_rw(),
                    None,
                    None,
                )
            }
        }
    }

    /// 同步所有文件映射
    pub fn sync_all_file_mappings(&mut self) -> Result<(), PagingError> {
        for area in &self.areas {
            area.sync_file(&mut self.page_table)?;
        }
        Ok(())
    }
}

impl<PT: PageTableInner<E>, E: PageTableEntry> Default for MemorySpace<PT, E> {
    fn default() -> Self {
        Self::new()
    }
}
