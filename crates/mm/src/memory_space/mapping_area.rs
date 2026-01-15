use alloc::collections::btree_map::BTreeMap;
use core::cmp::min;

use crate::arch_ops::{arch_ops, TlbBatchContextWrapper};
use crate::mm_config;
use crate::address::{Paddr, PageNum, Ppn, UsizeConvert, Vpn, VpnRange};
use crate::frame_allocator::{TrackedFrames, alloc_frame};
use crate::memory_space::MmapFile;
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
}

/// 内存区域的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    KernelText,   // 内核代码段
    KernelRodata, // 内核只读数据段
    KernelData,   // 内核数据段
    KernelStack,  // 内核栈
    KernelBss,    // 内核 BSS 段
    KernelHeap,   // 内核堆
    KernelMmio,   // 内核内存映射 I/O
    UserText,     // 用户代码段
    UserRodata,   // 用户只读数据段
    UserData,     // 用户数据段
    UserBss,      // 用户 BSS 段
    UserStack,    // 用户栈
    UserHeap,     // 用户堆
    UserMmap,     // 用户 mmap 匿名映射
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
    pub fn vpn_range(&self) -> VpnRange {
        self.vpn_range
    }

    pub fn permission(&self) -> UniversalPTEFlag {
        self.permission.clone()
    }

    pub fn set_permission(&mut self, perm: UniversalPTEFlag) {
        self.permission = perm;
    }

    pub fn map_type(&self) -> MapType {
        self.map_type
    }

    pub fn area_type(&self) -> AreaType {
        self.area_type
    }

    /// 已实际映射的页数（仅对 Framed 有意义）
    pub fn mapped_pages(&self) -> usize {
        match self.map_type {
            MapType::Framed => self
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
        self.frames.get(&vpn).map(|tracked| match tracked {
            TrackedFrames::Single(frame) => frame.ppn(),
            TrackedFrames::Multiple(frames) => frames.first().map(|f| f.ppn()).unwrap(),
            TrackedFrames::Contiguous(_) => {
                panic!("当前实现不支持连续帧");
            }
        })
    }

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
        let ppn = match self.map_type {
            MapType::Direct => {
                let vaddr = vpn.start_addr();
                let paddr = unsafe { arch_ops().vaddr_to_paddr(vaddr.as_usize()) };
                Ppn::from_addr_floor(Paddr::from_usize(paddr))
            }
            MapType::Framed => {
                let frame = alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
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
        page_table.unmap_with_batch(vpn, batch)?;

        if self.map_type == MapType::Framed {
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
    pub fn copy_data<PT: PageTableInner<E>, E: PageTableEntry>(&self, page_table: &mut PT, data: &[u8], offset: usize) {
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

    /// 克隆映射区域及其数据
    pub fn clone_with_data<PT: PageTableInner<E>, E: PageTableEntry>(
        &self,
        page_table: &mut PT,
    ) -> Result<Self, page_table::PagingError> {
        let mut new_area = self.clone_metadata();
        if self.map_type != MapType::Framed {
            return Err(page_table::PagingError::UnsupportedMapType);
        }

        let page_size = mm_config().page_size();

        TlbBatchContextWrapper::execute(|batch| {
            for (vpn, tracked_frames) in &self.frames {
                match tracked_frames {
                    TrackedFrames::Single(frame) => {
                        let new_frame =
                            alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;

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
                            let new_frame =
                                alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
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

                            new_frames.push(new_frame);
                        }

                        new_area
                            .frames
                            .insert(*vpn, TrackedFrames::Multiple(new_frames));
                    }
                    TrackedFrames::Contiguous(_) => {
                        return Err(page_table::PagingError::HugePageSplitNotImplemented);
                    }
                }
            }

            Ok(new_area)
        })
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
                MapType::Framed
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
            MapType::Framed => {
                if wants_mapping {
                    TlbBatchContextWrapper::execute(|batch| {
                        for vpn in VpnRange::new(change_start, change_end) {
                            page_table.update_flags_with_batch(vpn, middle_area.permission.clone(), Some(batch))?;
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
                            let frame =
                                alloc_frame().ok_or(page_table::PagingError::FrameAllocFailed)?;
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
