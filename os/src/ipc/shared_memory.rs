//! 共享内存模块
//!
//! 提供共享物理页的分配与映射到当前进程用户空间的能力。
//!
//! # 设计概览
//!
//! - [`SharedMemory`] 持有一组物理页（`FrameTracker`），生命周期由 RAII 管理。
//! - [`SharedMemoryTable`] 以 `Vec<Arc<SharedMemory>>` 的形式登记若干共享段，仅提供
//!   “创建/移除登记项/计数”等最小管理能力。
//!
//! # 注意事项
//!
//! - [`SharedMemoryTable::remove`] 当前只会从表中移除登记项，不会自动取消已建立的映射。
//! - [`SharedMemory::map_to_user`] 以“当前任务的用户空间”为目标建立映射；当前签名为
//!   `self`（按值），调用方需确保拥有该共享段的所有权。

use alloc::{sync::Arc, vec::Vec};

use crate::{
    config::PAGE_SIZE,
    kernel::{current_cpu, current_task},
    mm::{
        frame_allocator::{FrameTracker, alloc_frames},
        page_table::{PagingError, UniversalPTEFlag},
    },
};

/// 共享内存表：简单管理若干共享段
pub struct SharedMemoryTable {
    memory: Vec<Arc<SharedMemory>>,
}

impl SharedMemoryTable {
    /// 创建一个空的共享内存表。
    pub fn new() -> Self {
        Self { memory: Vec::new() }
    }

    /// 新建共享段并登记，返回 `Arc` 句柄。
    ///
    /// `pages` 为共享段占用的物理页数。
    pub fn create(&mut self, pages: usize) -> Arc<SharedMemory> {
        let shm = Arc::new(SharedMemory::new(pages));
        self.memory.push(shm.clone());
        shm
    }

    /// 从表中移除共享段。
    ///
    /// 注意：该操作仅从表中移除登记项；若仍有其他地方持有 `Arc`，
    /// 共享段不会真正释放。
    pub fn remove(&mut self, shm: &Arc<SharedMemory>) -> bool {
        if let Some(i) = self.memory.iter().position(|x| Arc::ptr_eq(x, shm)) {
            self.memory.swap_remove(i);
            // XXX: 是不是还应取消在当前进程用户空间上的映射
            true
        } else {
            false
        }
    }

    /// 当前已登记的共享段数量。
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// 表是否为空。
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }
}

/// 共享内存：持有一组物理页（FrameTracker）
pub struct SharedMemory {
    frames: Vec<FrameTracker>,
    len: usize,
}

impl SharedMemory {
    /// 分配 `pages` 个物理页作为共享段。
    pub fn new(pages: usize) -> Self {
        let frames = alloc_frames(pages).expect("unable to alloc shared memory");
        SharedMemory {
            frames,
            len: pages * PAGE_SIZE,
        }
    }

    /// 共享段字节数。
    pub fn len(&self) -> usize {
        self.len
    }

    /// 共享段是否为空（长度是否为 0）。
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 将共享段映射到当前进程用户空间。
    ///
    /// # 返回值
    ///
    /// - `Ok(addr)`：映射起始虚拟地址
    /// - `Err(PagingError)`：映射失败
    ///
    /// # 注意
    ///
    /// 当前实现会为共享段申请一段可读写、用户可访问的映射区域。
    pub fn map_to_user(self) -> Result<usize, PagingError> {
        let current = current_task();
        let mut task = current.lock();
        let space = task
            .memory_space
            .as_mut()
            .expect("map_to_user_at: task has no user memory space");

        let flags = UniversalPTEFlag::READABLE
            | UniversalPTEFlag::WRITEABLE
            | UniversalPTEFlag::USER_ACCESSIBLE
            | UniversalPTEFlag::VALID;

        space.lock().mmap(0, self.len, flags)
    }
}
