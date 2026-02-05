use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;
use core::cmp::min;

use crate::address::{Paddr, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
use crate::arch_ops::{TlbBatchContextWrapper, arch_ops};
use crate::frame_allocator::{FrameTracker, TrackedFrames, alloc_contig_frames_aligned, alloc_frame};
use crate::memory_space::MmapFile;
use crate::mm_config;
use crate::page_table::{self, PageSize, PageTableEntry, PageTableInner, UniversalPTEFlag};
use uapi::mm::MapFlags;

/// 映射策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// 直接映射（虚拟地址 = 物理地址 + VIRTUAL_BASE）
    Direct,
    /// 帧映射（从帧分配器分配）
    Framed,
    /// 保留地址范围（不建立页表映射）
    ///
    /// 用于实现 PROT_NONE（guard page / no-access VMA）语义：
    /// - mmap(PROT_NONE) 需要"成功占位"但不应该映射可访问页表项
    /// - mprotect(PROT_NONE) 会把原有页表映射解除并转为 Reserved
    Reserved,
    /// 延迟分配（Demand Paging）：先保留虚拟地址范围，首次访问再分配物理页并建立映射。
    ///
    /// 与 [`MapType::Reserved`] 不同，Lazy 区域有“可访问的权限语义”，只是尚未 materialize
    /// 具体页表项；缺页时应按权限分配/装载对应页。
    Lazy,
}

/// 内存区域的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    /// 内核代码段
    KernelText,
    /// 内核只读数据段
    KernelRodata,
    /// 内核数据段
    KernelData,
    /// 内核栈
    KernelStack,
    /// 内核 BSS 段
    KernelBss,
    /// 内核堆
    KernelHeap,
    /// 内核内存映射 I/O 区域（MMIO）
    KernelMmio,
    /// 用户代码段
    UserText,
    /// 用户只读数据段
    UserRodata,
    /// 用户数据段
    UserData,
    /// 用户 BSS 段
    UserBss,
    /// 用户栈
    UserStack,
    /// 用户堆
    UserHeap,
    /// 用户 mmap 匿名映射区域
    UserMmap,
}

/// 内存空间中的一个内存映射区域
#[derive(Debug)]
pub struct MappingArea {
    /// 此映射区域的虚拟页号范围
    vpn_range: VpnRange,
    /// 此映射区域的类型
    area_type: AreaType,
    /// 映射策略类型
    map_type: MapType,
    /// 此映射区域的权限（使用 UniversalPTEFlag 以提高性能）
    permission: UniversalPTEFlag,
    /// 用于帧映射区域的跟踪帧
    frames: BTreeMap<Vpn, TrackedFrames>,
    /// 文件映射信息（如果是文件映射）
    file: Option<MmapFile>,
}

impl MappingArea {
    /// Find a contiguous tracked range covering `vpn`, if any.
    ///
    /// The returned base VPN is the key stored in `self.frames` (typically 2MiB-aligned for THP).
    fn find_contiguous_covering(&self, vpn: Vpn) -> Option<(Vpn, &Arc<crate::frame_allocator::FrameRangeTracker>)> {
        let (base, tracked) = self.frames.range(..=vpn).next_back()?;
        match tracked {
            TrackedFrames::Contiguous(range) => {
                let base = *base;
                let end = Vpn::from_usize(base.as_usize().saturating_add(range.len()));
                if vpn >= base && vpn < end {
                    Some((base, range))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Whether `vpn` has been materialized (tracked) either as a 4K page or as part of a
    /// contiguous tracked range (e.g. a collapsed THP chunk).
    fn is_materialized(&self, vpn: Vpn) -> bool {
        self.frames.contains_key(&vpn) || self.find_contiguous_covering(vpn).is_some()
    }

    /// 返回该区域覆盖的虚拟页号范围。
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    /// 返回该区域当前权限标志。
    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission.clone()
    }

    /// 设置该区域权限标志。
    pub fn set_permission(&mut self, perm: UniversalPTEFlag) {
        self.permission = perm;
    }

    /// 返回该区域的映射策略（直接映射/帧映射/保留）。
    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    /// 返回该区域的语义类型（内核段/用户段等）。
    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    /// 已实际映射的页数（仅对 Framed 有意义）
    pub fn mapped_pages(&self) -> usize {
        match self.map_type {
            MapType::Framed | MapType::Lazy => self
                .frames
                .values()
                .map(|t| match t {
                    TrackedFrames::Single(_) => 1,
                    TrackedFrames::Multiple(v) => v.len(),
                    TrackedFrames::Contiguous(r) => r.len(),
                })
                .sum(),
            _ => 0,
        }
    }

    /// 获取虚拟页号（VPN）对应的物理页号（PPN）（如果已映射）
    pub fn get_ppn(&self, vpn: Vpn) -> Option<crate::address::Ppn> {
        if let Some(tracked) = self.frames.get(&vpn) {
            return Some(match tracked {
                TrackedFrames::Single(frame) => frame.ppn(),
                TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                TrackedFrames::Contiguous(range) => range.start_ppn(),
            });
        }

        // Fallback: a collapsed huge chunk is tracked at its base VPN only.
        let (base, range) = self.find_contiguous_covering(vpn)?;
        let off = vpn.as_usize() - base.as_usize();
        Some(Ppn::from_usize(range.start_ppn().as_usize() + off))
    }

    /// 创建一个新的映射区域描述符。
    pub fn new(
        vpn_range: VpnRange,
        area_type: AreaType,
        map_type: MapType,
        permission: UniversalPTEFlag,
        file: Option<MmapFile>,
    ) -> Self {
        MappingArea {
            vpn_range,
            area_type,
            map_type,
            permission,
            frames: BTreeMap::new(),
            file,
        }
    }

    /// 映射单个虚拟页到物理页
    pub fn map_one<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        self.map_one_with_batch(page_table, vpn, None)
    }

    /// 映射单个虚拟页到物理页（支持批处理）
    fn map_one_with_batch<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContextWrapper>,
    ) -> Result<(), page_table::PagingError> {
        // For demand paging, mapping the same page twice should be a no-op.
        if matches!(self.map_type, MapType::Framed | MapType::Lazy) && self.is_materialized(vpn) {
            return Ok(());
        }

        let ppn = match self.map_type {
            MapType::Direct => {
                let vaddr = vpn.start_addr();
                let paddr = unsafe { arch_ops().vaddr_to_paddr(vaddr.as_usize()) };
                Ppn::from_addr_floor(Paddr::from_usize(paddr))
            }
            MapType::Framed | MapType::Lazy => {
                let frame = Arc::new(alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?);
                let ppn = frame.ppn();
                self.frames.insert(vpn, TrackedFrames::Single(frame));
                ppn
            }
            MapType::Reserved => {
                return Ok(());
            }
        };

        page_table.map_with_batch(vpn, ppn, PageSize::Size4K, self.permission.clone(), batch)?;
        Ok(())
    }

    /// 映射此映射区域中的所有页
    pub fn map<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
    ) -> Result<(), page_table::PagingError> {
        if matches!(self.map_type, MapType::Reserved | MapType::Lazy) {
            // Reserved: never map.
            // Lazy: mappings are created on-demand via page fault handling.
            return Ok(());
        }
        TlbBatchContextWrapper::execute(|batch| {
            for vpn in self.vpn_range {
                self.map_one_with_batch(page_table, vpn, Some(batch))?;
            }
            Ok(())
        })
    }

    /// 解除映射单个虚拟页
    pub fn unmap_one<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        self.unmap_one_with_batch(page_table, vpn, None)
    }

    /// 解除映射单个虚拟页（支持批处理）
    fn unmap_one_with_batch<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        vpn: Vpn,
        batch: Option<&mut TlbBatchContextWrapper>,
    ) -> Result<(), page_table::PagingError> {
        if self.map_type == MapType::Reserved {
            return Ok(());
        }
        if self.map_type == MapType::Lazy && !self.frames.contains_key(&vpn) {
            // A collapsed huge chunk is tracked at its base VPN only, and must be split before
            // doing per-page unmap operations.
            if self.find_contiguous_covering(vpn).is_some() {
                return Err(page_table::PagingError::HugePageSplitNotImplemented);
            }
            return Ok(());
        }
        page_table.unmap_with_batch(vpn, batch)?;

        if matches!(self.map_type, MapType::Framed | MapType::Lazy) {
            self.frames.remove(&vpn);
        }
        Ok(())
    }

    /// 解除映射此映射区域中的所有页
    pub fn unmap<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
    ) -> Result<(), page_table::PagingError> {
        TlbBatchContextWrapper::execute(|batch| {
            for vpn in self.vpn_range {
                self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
            }
            Ok(())
        })
    }

    /// 复制数据到已映射的区域
    pub fn copy_data<PT: PageTableInner<E>, E: PageTableEntry>(
        &self,
        page_table: &mut PT,
        data: &[u8],
        offset: usize,
    ) {
        let mut copied = 0;
        let total_len = data.len();
        let page_size = mm_config().page_size();

        for (i, vpn) in self.vpn_range.iter().enumerate() {
            if copied >= total_len {
                break;
            }

            let vaddr = vpn.start_addr();
            let paddr = page_table.translate(vaddr).expect("无法转换虚拟地址");
            let paddr = if i == 0 {
                paddr.as_usize().checked_add(offset).unwrap()
            } else {
                paddr.as_usize()
            };

            let page_capacity = if i == 0 {
                page_size - offset
            } else {
                page_size
            };

            let remaining = total_len - copied;
            let to_copy = min(remaining, page_capacity);

            unsafe {
                let dst_va = arch_ops().paddr_to_vaddr(paddr);
                let dst = dst_va as *mut u8;
                let src = data.as_ptr().add(copied);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }

            copied += to_copy;
        }
    }

    /// 克隆元数据，但不克隆帧
    pub fn clone_metadata(&self) -> Self {
        MappingArea {
            vpn_range: self.vpn_range,
            area_type: self.area_type,
            map_type: self.map_type,
            permission: self.permission.clone(),
            frames: BTreeMap::new(),
            file: self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset,
                len: f.len,
                prot: f.prot,
                flags: f.flags,
            }),
        }
    }

    /// 克隆区域并共享已分配的帧（用于 COW / fork 等共享语义）。
    ///
    /// 注意：该方法只共享 `frames` 中已 materialize 的页；Lazy 区域中未分配的页仍保持未分配状态。
    pub fn clone_shared_frames(&self) -> Self {
        let mut out = self.clone_metadata();
        out.frames = self.frames.clone();
        out
    }

    /// 返回当前区域中已 materialize 的 VPN 列表。
    ///
    /// - Framed：通常等于 vpn_range 的全部页
    /// - Lazy：仅包含已被访问并分配的页
    pub fn mapped_vpns(&self) -> alloc::vec::Vec<Vpn> {
        self.frames.keys().copied().collect()
    }

    /// 返回该区域的文件映射信息（若存在）。
    pub fn mmap_file(&self) -> Option<&MmapFile> {
        self.file.as_ref()
    }

    /// 将已跟踪的物理帧映射到给定页表中（不会分配新帧，仅建立 PTE）。
    pub fn map_tracked_frames<PT: PageTableInner<E>, E: PageTableEntry>(
        &self,
        page_table: &mut PT,
        flags: UniversalPTEFlag,
    ) -> Result<(), page_table::PagingError> {
        for (vpn, tracked) in &self.frames {
            match tracked {
                TrackedFrames::Single(frame) => {
                    page_table.map(*vpn, frame.ppn(), PageSize::Size4K, flags)?;
                }
                TrackedFrames::Multiple(frames) => {
                    let ppn = frames.first().map(|f| f.ppn()).unwrap();
                    page_table.map(*vpn, ppn, PageSize::Size4K, flags)?;
                }
                TrackedFrames::Contiguous(range) => {
                    // THP huge chunk: map as a 2MiB leaf at its base VPN.
                    page_table.map(*vpn, range.start_ppn(), PageSize::Size2M, flags)?;
                }
            }
        }
        Ok(())
    }

    /// 克隆映射区域及其数据
    pub fn clone_with_data<PT: PageTableInner<E>, E: PageTableEntry>(
        &self,
        page_table: &mut PT,
    ) -> Result<Self, page_table::PagingError> {
        let mut new_area = self.clone_metadata();
        if !matches!(self.map_type, MapType::Framed | MapType::Lazy) {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        let page_size = mm_config().page_size();

        TlbBatchContextWrapper::execute(|batch| {
            for (vpn, tracked_frames) in &self.frames {
                match tracked_frames {
                    TrackedFrames::Single(frame) => {
                        let new_frame = Arc::new(
                            alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?,
                        );

                        let new_ppn = new_frame.ppn();
                        let src_ppn = frame.ppn();

                        unsafe {
                            let src_va = arch_ops().paddr_to_vaddr(src_ppn.start_addr().as_usize());
                            let dst_va = arch_ops().paddr_to_vaddr(new_ppn.start_addr().as_usize());

                            core::ptr::copy_nonoverlapping(
                                src_va as *const u8,
                                dst_va as *mut u8,
                                page_size,
                            );
                        }

                        page_table.map_with_batch(
                            *vpn,
                            new_ppn,
                            PageSize::Size4K,
                            self.permission.clone(),
                            Some(batch),
                        )?;

                        new_area
                            .frames
                            .insert(*vpn, TrackedFrames::Single(new_frame));
                    }
                    TrackedFrames::Multiple(frames) => {
                        let mut new_frames = alloc::vec::Vec::new();

                        for frame in frames.iter() {
                            let new_frame = Arc::new(
                                alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?,
                            );
                            let new_ppn = new_frame.ppn();
                            let src_ppn = frame.ppn();

                            unsafe {
                                let src_va =
                                    arch_ops().paddr_to_vaddr(src_ppn.start_addr().as_usize());
                                let dst_va =
                                    arch_ops().paddr_to_vaddr(new_ppn.start_addr().as_usize());

                                core::ptr::copy_nonoverlapping(
                                    src_va as *const u8,
                                    dst_va as *mut u8,
                                    page_size,
                                );
                            }

                            page_table.map_with_batch(
                                *vpn,
                                new_ppn,
                                PageSize::Size4K,
                                self.permission.clone(),
                                Some(batch),
                            )?;

                            new_frames.push(new_frame);
                        }

                        new_area
                            .frames
                            .insert(*vpn, TrackedFrames::Multiple(new_frames));
                    }
                    TrackedFrames::Contiguous(range) => {
                        // Deep-copy a huge chunk.
                        //
                        // NOTE: We currently use contiguous tracking only for 2MiB THP chunks.
                        const THP_2M_PAGES: usize = 512;
                        if range.len() != THP_2M_PAGES {
                            return Err(page_table::PagingError::HugePageSplitNotImplemented);
                        }
                        let base_ppn = range.start_ppn();

                        let new_range = alloc_contig_frames_aligned(THP_2M_PAGES, THP_2M_PAGES)
                            .ok_or(page_table::PagingError::FrameAllocFailed)?;
                        let new_base_ppn = new_range.start_ppn();
                        let new_range = Arc::new(new_range);

                        for i in 0..THP_2M_PAGES {
                            let src_ppn = Ppn::from_usize(base_ppn.as_usize() + i);
                            let dst_ppn = Ppn::from_usize(new_base_ppn.as_usize() + i);
                            unsafe {
                                let src_va =
                                    arch_ops().paddr_to_vaddr(src_ppn.start_addr().as_usize());
                                let dst_va =
                                    arch_ops().paddr_to_vaddr(dst_ppn.start_addr().as_usize());
                                core::ptr::copy_nonoverlapping(
                                    src_va as *const u8,
                                    dst_va as *mut u8,
                                    page_size,
                                );
                            }
                        }

                        page_table.map_with_batch(
                            *vpn,
                            new_base_ppn,
                            PageSize::Size2M,
                            self.permission.clone(),
                            Some(batch),
                        )?;
                        new_area
                            .frames
                            .insert(*vpn, TrackedFrames::Contiguous(new_range));
                    }
                }
            }

            Ok(new_area)
        })
    }

    /// Split a tracked 2MiB contiguous chunk at `base_vpn` into 512 4KiB trackers.
    ///
    /// This only updates `self.frames` ownership model; the caller is responsible for splitting
    /// page table mappings beforehand (or afterwards) as needed.
    pub fn split_contiguous_2m_at(&mut self, base_vpn: Vpn) -> Result<(), page_table::PagingError> {
        const THP_2M_PAGES: usize = 512;

        let Some(tracked) = self.frames.remove(&base_vpn) else {
            return Ok(());
        };
        match tracked {
            TrackedFrames::Contiguous(range) => {
                if range.len() != THP_2M_PAGES {
                    self.frames.insert(base_vpn, TrackedFrames::Contiguous(range));
                    return Err(page_table::PagingError::HugePageSplitNotImplemented);
                }

                let owned = match Arc::try_unwrap(range) {
                    Ok(r) => r,
                    Err(r) => {
                        // Shared ownership: we can't transfer page ownership without a more
                        // sophisticated allocator model.
                        self.frames.insert(base_vpn, TrackedFrames::Contiguous(r));
                        return Err(page_table::PagingError::HugePageSplitNotImplemented);
                    }
                };

                let frames: alloc::vec::Vec<FrameTracker> = owned.into_allocated_frames();
                debug_assert!(frames.len() == THP_2M_PAGES);
                for (i, frame) in frames.into_iter().enumerate() {
                    let vpn = Vpn::from_usize(base_vpn.as_usize() + i);
                    self.frames.insert(vpn, TrackedFrames::Single(Arc::new(frame)));
                }
                Ok(())
            }
            other => {
                // Not a contiguous chunk; restore it.
                self.frames.insert(base_vpn, other);
                Ok(())
            }
        }
    }

    /// Resolve a COW write fault for a single 4K page.
    ///
    /// This assumes:
    /// - the VMA has logical WRITE permission (self.permission contains WRITEABLE)
    /// - a PTE exists but is currently write-protected (WRITEABLE cleared)
    pub fn resolve_cow_write_fault<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        vpn: Vpn,
    ) -> Result<(), page_table::PagingError> {
        if !self.permission.contains(UniversalPTEFlag::WRITEABLE) {
            return Err(page_table::PagingError::InvalidAddress);
        }

        let tracked = self
            .frames
            .get(&vpn)
            .ok_or(page_table::PagingError::NotMapped)?
            .clone();

        let page_size = mm_config().page_size();
        let new_flags = self.permission.clone();

        match tracked {
            TrackedFrames::Single(frame) => {
                if Arc::strong_count(&frame) == 1 {
                    // Exclusive owner: just restore WRITE permission.
                    page_table.update_flags_with_batch(vpn, new_flags, None)?;
                    return Ok(());
                }

                // Shared: allocate a private copy and remap.
                let new_frame =
                    Arc::new(alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?);
                let src_ppn = frame.ppn();
                let dst_ppn = new_frame.ppn();

                unsafe {
                    let src_va = arch_ops().paddr_to_vaddr(src_ppn.start_addr().as_usize());
                    let dst_va = arch_ops().paddr_to_vaddr(dst_ppn.start_addr().as_usize());
                    core::ptr::copy_nonoverlapping(src_va as *const u8, dst_va as *mut u8, page_size);
                }

                TlbBatchContextWrapper::execute(|batch| {
                    page_table.unmap_with_batch(vpn, Some(batch))?;
                    page_table.map_with_batch(
                        vpn,
                        dst_ppn,
                        PageSize::Size4K,
                        new_flags,
                        Some(batch),
                    )?;
                    Ok::<(), page_table::PagingError>(())
                })?;

                // Update ownership after we have successfully installed the new mapping.
                self.frames.insert(vpn, TrackedFrames::Single(new_frame));
                Ok(())
            }
            TrackedFrames::Multiple(_) | TrackedFrames::Contiguous(_) => {
                Err(page_table::PagingError::HugePageSplitNotImplemented)
            }
        }
    }

    /// 拆分区域为两部分
    pub fn split_at<PT: PageTableInner<E>, E: PageTableEntry>(
        mut self,
        _page_table: &mut PT,
        split_vpn: Vpn,
    ) -> Result<(Self, Self), page_table::PagingError> {
        if !self.vpn_range.contains(split_vpn) {
            return Err(page_table::PagingError::InvalidAddress);
        }

        if split_vpn == self.vpn_range.start() || split_vpn == self.vpn_range.end() {
            return Err(page_table::PagingError::InvalidAddress);
        }

        if self.map_type != MapType::Framed {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        let left_range = VpnRange::new(self.vpn_range.start(), split_vpn);
        let right_range = VpnRange::new(split_vpn, self.vpn_range.end());

        let left_pages = split_vpn.as_usize() - self.vpn_range.start().as_usize();
        let page_size = mm_config().page_size();

        let left_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset,
            len: left_pages * page_size,
            prot: f.prot,
            flags: f.flags,
        });

        let right_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + left_pages * page_size,
            len: f.len - left_pages * page_size,
            prot: f.prot,
            flags: f.flags,
        });

        let mut left_area = MappingArea::new(
            left_range,
            self.area_type,
            self.map_type,
            self.permission.clone(),
            left_file,
        );

        let mut right_area = MappingArea::new(
            right_range,
            self.area_type,
            self.map_type,
            self.permission.clone(),
            right_file,
        );

        let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
        for vpn in vpns {
            if let Some(tracked_frames) = self.frames.remove(&vpn) {
                if vpn < split_vpn {
                    left_area.frames.insert(vpn, tracked_frames);
                } else {
                    right_area.frames.insert(vpn, tracked_frames);
                }
            }
        }

        Ok((left_area, right_area))
    }

    /// 部分修改权限
    pub fn partial_change_permission<PT: PageTableInner<E>, E: PageTableEntry>(
        mut self,
        page_table: &mut PT,
        start_vpn: Vpn,
        end_vpn: Vpn,
        new_perm: UniversalPTEFlag,
    ) -> Result<alloc::vec::Vec<Self>, page_table::PagingError> {
        let area_start = self.vpn_range.start();
        let area_end = self.vpn_range.end();

        let change_start = core::cmp::max(start_vpn, area_start);
        let change_end = core::cmp::min(end_vpn, area_end);

        if change_start >= change_end {
            return Ok(alloc::vec![self]);
        }

        let wants_mapping = new_perm.intersects(
            UniversalPTEFlag::READABLE | UniversalPTEFlag::WRITEABLE | UniversalPTEFlag::EXECUTABLE,
        );

        let left_range = VpnRange::new(area_start, change_start);
        let middle_range = VpnRange::new(change_start, change_end);
        let right_range = VpnRange::new(change_end, area_end);

        let left_pages = change_start.as_usize() - area_start.as_usize();
        let middle_pages = change_end.as_usize() - change_start.as_usize();
        let page_size = mm_config().page_size();

        let left_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset,
            len: left_pages * page_size,
            prot: f.prot,
            flags: f.flags,
        });

        let middle_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + left_pages * page_size,
            len: middle_pages * page_size,
            prot: f.prot,
            flags: f.flags,
        });

        let right_file = self.file.as_ref().map(|f| MmapFile {
            file: f.file.clone(),
            offset: f.offset + (left_pages + middle_pages) * page_size,
            len: f.len - (left_pages + middle_pages) * page_size,
            prot: f.prot,
            flags: f.flags,
        });

        let mut left_area = if area_start < change_start {
            Some(MappingArea::new(
                left_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                left_file,
            ))
        } else {
            None
        };

        let mut middle_area = MappingArea::new(
            middle_range,
            self.area_type,
            if wants_mapping {
                if self.map_type == MapType::Lazy {
                    MapType::Lazy
                } else {
                    MapType::Framed
                }
            } else {
                MapType::Reserved
            },
            new_perm,
            middle_file,
        );

        let mut right_area = if change_end < area_end {
            Some(MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                right_file,
            ))
        } else {
            None
        };

        match self.map_type {
            MapType::Direct => return Err(page_table::PagingError::UnsupportedMapType),
            MapType::Framed | MapType::Lazy => {
                if wants_mapping {
                    TlbBatchContextWrapper::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            // Lazy may not have a PTE for this vpn yet.
                            if self.map_type == MapType::Lazy && !self.frames.contains_key(&vpn) {
                                continue;
                            }
                            page_table.update_flags_with_batch(
                                vpn,
                                middle_area.permission.clone(),
                                Some(batch),
                            )?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;

                    for vpn in VpnRange::new(change_start, change_end) {
                        if let Some(tracked) = self.frames.remove(&vpn) {
                            middle_area.frames.insert(vpn, tracked);
                        }
                    }
                } else {
                    TlbBatchContextWrapper::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;
                }

                let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
                for vpn in vpns {
                    if let Some(tracked) = self.frames.remove(&vpn) {
                        if vpn < change_start {
                            if let Some(ref mut l) = left_area {
                                l.frames.insert(vpn, tracked);
                            }
                        } else if vpn >= change_end {
                            if let Some(ref mut r) = right_area {
                                r.frames.insert(vpn, tracked);
                            }
                        }
                    }
                }
            }
            MapType::Reserved => {
                if wants_mapping {
                    TlbBatchContextWrapper::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            let frame = Arc::new(
                                alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?,
                            );
                            let ppn = frame.ppn();
                            middle_area.frames.insert(vpn, TrackedFrames::Single(frame));
                            page_table.map_with_batch(
                                vpn,
                                ppn,
                                PageSize::Size4K,
                                middle_area.permission.clone(),
                                Some(batch),
                            )?;
                        }
                        Ok::<(), page_table::PagingError>(())
                    })?;
                }
            }
        }

        let mut out = alloc::vec::Vec::new();
        if let Some(l) = left_area {
            out.push(l);
        }
        out.push(middle_area);
        if let Some(r) = right_area {
            out.push(r);
        }
        Ok(out)
    }

    /// 部分解除映射
    pub fn partial_unmap<PT: PageTableInner<E>, E: PageTableEntry>(
        mut self,
        page_table: &mut PT,
        start_vpn: Vpn,
        end_vpn: Vpn,
    ) -> Result<Option<(Self, Option<Self>)>, page_table::PagingError> {
        let area_start = self.vpn_range.start();
        let area_end = self.vpn_range.end();

        let unmap_start = core::cmp::max(start_vpn, area_start);
        let unmap_end = core::cmp::min(end_vpn, area_end);

        if unmap_start >= unmap_end {
            return Ok(Some((self, None)));
        }

        if self.map_type != MapType::Reserved {
            TlbBatchContextWrapper::execute(|batch| {
                for vpn in VpnRange::new(unmap_start, unmap_end) {
                    self.unmap_one_with_batch(page_table, vpn, Some(batch))?;
                }
                Ok::<(), page_table::PagingError>(())
            })?;
        }

        if unmap_start == area_start && unmap_end == area_end {
            return Ok(None);
        } else if unmap_start == area_start {
            self.vpn_range = VpnRange::new(unmap_end, area_end);
            return Ok(Some((self, None)));
        } else if unmap_end == area_end {
            self.vpn_range = VpnRange::new(area_start, unmap_start);
            return Ok(Some((self, None)));
        } else {
            let left_range = VpnRange::new(area_start, unmap_start);
            let right_range = VpnRange::new(unmap_end, area_end);

            let left_pages = unmap_start.as_usize() - area_start.as_usize();
            let middle_pages = unmap_end.as_usize() - unmap_start.as_usize();
            let page_size = mm_config().page_size();

            let left_file = self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset,
                len: left_pages * page_size,
                prot: f.prot,
                flags: f.flags,
            });

            let right_file = self.file.as_ref().map(|f| MmapFile {
                file: f.file.clone(),
                offset: f.offset + (left_pages + middle_pages) * page_size,
                len: f.len - (left_pages + middle_pages) * page_size,
                prot: f.prot,
                flags: f.flags,
            });

            let mut left_area = MappingArea::new(
                left_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                left_file,
            );

            let mut right_area = MappingArea::new(
                right_range,
                self.area_type,
                self.map_type,
                self.permission.clone(),
                right_file,
            );

            let vpns: alloc::vec::Vec<Vpn> = self.frames.keys().copied().collect();
            for vpn in vpns {
                if let Some(tracked_frames) = self.frames.remove(&vpn) {
                    if vpn < unmap_start {
                        left_area.frames.insert(vpn, tracked_frames);
                    } else if vpn >= unmap_end {
                        right_area.frames.insert(vpn, tracked_frames);
                    }
                }
            }

            return Ok(Some((left_area, Some(right_area))));
        }
    }

    /// 从文件加载数据到已分配的物理页中
    pub fn load_from_file(&mut self) -> Result<(), page_table::PagingError> {
        if let Some(ref mmap_file) = self.file {
            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();
            let page_size = mm_config().page_size();

            for (vpn, tracked_frame) in &self.frames {
                let page_offset = vpn.as_usize() - start_vpn.as_usize();
                let file_offset = mmap_file.offset + page_offset * page_size;

                let ppn = match tracked_frame {
                    TrackedFrames::Single(frame) => frame.ppn(),
                    TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                    TrackedFrames::Contiguous(_) => {
                        panic!("当前实现不支持连续帧");
                    }
                };

                let paddr = ppn.start_addr();
                let kernel_vaddr = arch_ops().paddr_to_vaddr(paddr.as_usize());
                let buffer =
                    unsafe { core::slice::from_raw_parts_mut(kernel_vaddr as *mut u8, page_size) };

                let read_len = min(
                    page_size,
                    mmap_file.len.saturating_sub(page_offset * page_size),
                );

                if read_len == 0 {
                    continue;
                }

                let actual_read = inode
                    .read_at(file_offset, &mut buffer[..read_len])
                    .map_err(|_| page_table::PagingError::InvalidAddress)?;

                if actual_read < read_len {
                    log::warn!(
                        "Partial read at offset {}: expected {}, got {}",
                        file_offset,
                        read_len,
                        actual_read
                    );
                }
            }
        }
        Ok(())
    }

    /// 将脏页写回文件
    pub fn sync_file<PT: PageTableInner<E>, E: PageTableEntry>(
        &self,
        page_table: &mut PT,
    ) -> Result<(), page_table::PagingError> {
        if let Some(ref mmap_file) = self.file {
            if !mmap_file.flags.contains(MapFlags::SHARED) {
                return Ok(());
            }

            let inode = mmap_file
                .file
                .inode()
                .map_err(|_| page_table::PagingError::InvalidAddress)?;
            let start_vpn = self.vpn_range.start();
            let page_size = mm_config().page_size();

            TlbBatchContextWrapper::execute(|batch| {
                for (vpn, tracked_frame) in &self.frames {
                    let (_, _, flags) = match page_table.walk(*vpn) {
                        Ok(result) => result,
                        Err(_) => continue,
                    };

                    if !flags.contains(UniversalPTEFlag::DIRTY) {
                        continue;
                    }

                    let page_offset = vpn.as_usize() - start_vpn.as_usize();
                    let file_offset = mmap_file.offset + page_offset * page_size;

                    let ppn = match tracked_frame {
                        TrackedFrames::Single(frame) => frame.ppn(),
                        TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                        TrackedFrames::Contiguous(_) => {
                            panic!("当前实现不支持连续帧");
                        }
                    };

                    let paddr = ppn.start_addr();
                    let kernel_vaddr = arch_ops().paddr_to_vaddr(paddr.as_usize());
                    let buffer = unsafe {
                        core::slice::from_raw_parts(kernel_vaddr as *const u8, page_size)
                    };

                    let write_len = min(
                        page_size,
                        mmap_file.len.saturating_sub(page_offset * page_size),
                    );

                    if write_len == 0 {
                        continue;
                    }

                    let actual_written = inode
                        .write_at(file_offset, &buffer[..write_len])
                        .map_err(|_| page_table::PagingError::InvalidAddress)?;

                    if actual_written != write_len {
                        log::error!(
                            "Partial write at offset {}: expected {}, got {}",
                            file_offset,
                            write_len,
                            actual_written
                        );
                        return Err(page_table::PagingError::InvalidAddress);
                    }

                    page_table.update_flags_with_batch(
                        *vpn,
                        flags & !UniversalPTEFlag::DIRTY,
                        Some(batch),
                    )?;
                }
                Ok(())
            })
        } else {
            Ok(())
        }
    }

    /// Try to collapse a 2MiB-aligned chunk into a single 2MiB leaf mapping (THP).
    ///
    /// This is an opportunistic optimization and returns `Ok(false)` if the chunk is not
    /// eligible or the threshold is not reached.
    pub fn try_collapse_2m<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        base_vpn: Vpn,
        min_present_pages: usize,
        avoid_shared_pages: bool,
    ) -> Result<bool, page_table::PagingError> {
        const THP_2M_PAGES: usize = 512;

        if self.map_type != MapType::Lazy {
            return Ok(false);
        }
        if self.file.is_some() {
            // No page cache yet; keep file mappings as 4K pages.
            return Ok(false);
        }
        if (base_vpn.as_usize() & (THP_2M_PAGES - 1)) != 0 {
            return Ok(false);
        }
        let chunk_end = Vpn::from_usize(base_vpn.as_usize() + THP_2M_PAGES);
        if base_vpn < self.vpn_range.start() || chunk_end > self.vpn_range.end() {
            return Ok(false);
        }

        // Skip if already a huge leaf.
        if let Ok((_ppn, size, _flags)) = page_table.walk(base_vpn) {
            if size == PageSize::Size2M {
                return Ok(false);
            }
        }

        let mut present = 0usize;
        for i in 0..THP_2M_PAGES {
            let v = Vpn::from_usize(base_vpn.as_usize() + i);
            if self.frames.contains_key(&v) {
                present += 1;
            }
        }
        if present < min_present_pages {
            return Ok(false);
        }

        if avoid_shared_pages {
            for i in 0..THP_2M_PAGES {
                let v = Vpn::from_usize(base_vpn.as_usize() + i);
                let Some(tracked) = self.frames.get(&v) else { continue };
                match tracked {
                    TrackedFrames::Single(frame) => {
                        if Arc::strong_count(frame) > 1 {
                            return Ok(false);
                        }
                    }
                    // Not expected in current code paths; be conservative.
                    TrackedFrames::Multiple(_) | TrackedFrames::Contiguous(_) => return Ok(false),
                }
            }
        }

        // Allocate a 2MiB-aligned contiguous backing.
        let range = alloc_contig_frames_aligned(THP_2M_PAGES, THP_2M_PAGES)
            .ok_or(page_table::PagingError::FrameAllocFailed)?;
        let base_ppn = range.start_ppn();
        let range = Arc::new(range);

        let page_size = mm_config().page_size();

        // Copy present 4K pages into the new backing. Missing pages stay zeroed.
        for i in 0..THP_2M_PAGES {
            let v = Vpn::from_usize(base_vpn.as_usize() + i);
            let Some(tracked) = self.frames.get(&v) else { continue };
            let src_ppn = match tracked {
                TrackedFrames::Single(frame) => frame.ppn(),
                TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
                TrackedFrames::Contiguous(_) => {
                    return Err(page_table::PagingError::HugePageSplitNotImplemented);
                }
            };

            let dst_ppn = Ppn::from_usize(base_ppn.as_usize() + i);

            unsafe {
                let src_va = arch_ops().paddr_to_vaddr(src_ppn.start_addr().as_usize());
                let dst_va = arch_ops().paddr_to_vaddr(dst_ppn.start_addr().as_usize());
                core::ptr::copy_nonoverlapping(src_va as *const u8, dst_va as *mut u8, page_size);
            }
        }

        // Switch page table mappings: unmap 4K leaves and install a 2MiB leaf.
        let flags = self.permission.clone();
        TlbBatchContextWrapper::execute(|batch| {
            for i in 0..THP_2M_PAGES {
                let v = Vpn::from_usize(base_vpn.as_usize() + i);
                let _ = page_table.unmap_with_batch(v, Some(batch));
            }
            page_table.map_with_batch(
                base_vpn,
                base_ppn,
                PageSize::Size2M,
                flags,
                Some(batch),
            )?;
            Ok::<(), page_table::PagingError>(())
        })?;

        // Replace per-page trackers with a single contiguous tracker.
        for i in 0..THP_2M_PAGES {
            let v = Vpn::from_usize(base_vpn.as_usize() + i);
            let _ = self.frames.remove(&v);
        }
        self.frames
            .insert(base_vpn, TrackedFrames::Contiguous(range));

        Ok(true)
    }
}

/// 动态扩展和收缩
impl MappingArea {
    /// 通过在末尾添加页来扩展区域（仅限 4K 页）
    pub fn extend<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() + count);

        if self.map_type == MapType::Lazy {
            // Demand-paged area: only extend the virtual range. Pages will be materialized on
            // page faults.
            self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);
            return Ok(new_end);
        }

        for i in 0..count {
            let vpn = Vpn::from_usize(old_end.as_usize() + i);
            self.map_one(page_table, vpn)?;
        }

        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }

    /// 通过从末尾移除页来收缩区域（仅限 4K 页）
    pub fn shrink<PT: PageTableInner<E>, E: PageTableEntry>(
        &mut self,
        page_table: &mut PT,
        count: usize,
    ) -> Result<Vpn, page_table::PagingError> {
        if count > self.vpn_range.len() {
            return Err(page_table::PagingError::ShrinkBelowStart);
        }

        let old_end = self.vpn_range.end();
        let new_end = Vpn::from_usize(old_end.as_usize() - count);

        for i in 0..count {
            let vpn = Vpn::from_usize(new_end.as_usize() + i);
            self.unmap_one(page_table, vpn)?;
        }

        self.vpn_range = VpnRange::new(self.vpn_range.start(), new_end);

        Ok(new_end)
    }
}
