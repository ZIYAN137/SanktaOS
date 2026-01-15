//! 架构相关内存管理操作 trait 定义和注册

use crate::page_table::PagingError;
use core::sync::atomic::{AtomicUsize, Ordering};

/// 架构相关内存管理操作
///
/// 此 trait 抽象了架构特定的内存操作，包括地址转换和 TLB 管理。
/// os crate 需要为具体架构实现此 trait。
pub trait ArchMmOps: Send + Sync {
    /// 将虚拟地址转换为物理地址（直接映射区域）
    ///
    /// # Safety
    /// 调用者必须确保虚拟地址已经映射
    unsafe fn vaddr_to_paddr(&self, vaddr: usize) -> usize;

    /// 将物理地址转换为虚拟地址（直接映射区域）
    fn paddr_to_vaddr(&self, paddr: usize) -> usize;

    /// 获取 sigreturn trampoline 代码字节
    fn sigreturn_trampoline_bytes(&self) -> &'static [u8];

    /// 获取 CPU 数量（用于 TLB shootdown）
    fn num_cpus(&self) -> usize;

    /// 发送 TLB flush IPI 到所有 CPU
    fn send_tlb_flush_ipi_all(&self);

    /// 创建 TLB 批处理上下文
    fn create_tlb_batch_context(&self) -> TlbBatchContextWrapper;
}

/// TLB 批处理上下文 trait
///
/// 用于批量处理 TLB 刷新操作，减少 IPI 开销
pub trait TlbBatchContextTrait {
    /// 刷新所有待处理的 TLB 条目
    fn flush(&mut self);
}

/// TLB 批处理上下文包装器
///
/// 包装架构特定的 TlbBatchContext 实现
pub struct TlbBatchContextWrapper {
    // 存储 fat pointer 的两部分
    inner_data: usize,
    inner_vtable: usize,
    // 存储实际的上下文数据（最多 64 字节应该足够）
    _storage: [u8; 64],
}

impl TlbBatchContextWrapper {
    /// 创建新的批处理上下文包装器
    ///
    /// # Safety
    /// 调用者必须确保 inner 指向有效的 TlbBatchContextTrait 实现
    pub unsafe fn new<T: TlbBatchContextTrait + 'static>(ctx: T) -> Self {
        unsafe {
            let mut wrapper = Self {
                inner_data: 0,
                inner_vtable: 0,
                _storage: [0u8; 64],
            };
            // 将 ctx 复制到 _storage 中
            assert!(core::mem::size_of::<T>() <= 64);
            let storage_ptr = wrapper._storage.as_mut_ptr() as *mut T;
            core::ptr::write(storage_ptr, ctx);
            // 设置 inner 指向 storage 中的数据
            let fat_ptr = storage_ptr as *mut dyn TlbBatchContextTrait;
            let (data, vtable) = core::mem::transmute::<*mut dyn TlbBatchContextTrait, (usize, usize)>(fat_ptr);
            wrapper.inner_data = data;
            wrapper.inner_vtable = vtable;
            wrapper
        }
    }

    /// 刷新所有待处理的 TLB 条目
    pub fn flush(&mut self) {
        if self.inner_data != 0 {
            unsafe {
                let fat_ptr = core::mem::transmute::<(usize, usize), *mut dyn TlbBatchContextTrait>(
                    (self.inner_data, self.inner_vtable)
                );
                (*fat_ptr).flush()
            }
        }
    }

    /// 在批处理上下文中执行操作
    pub fn execute<F, R>(f: F) -> Result<R, PagingError>
    where
        F: FnOnce(&mut Self) -> Result<R, PagingError>,
    {
        let mut ctx = arch_ops().create_tlb_batch_context();
        let result = f(&mut ctx);
        ctx.flush();
        result
    }
}

static ARCH_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static ARCH_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册架构操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_arch_ops(ops: &'static dyn ArchMmOps) {
    let ptr = ops as *const dyn ArchMmOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) =
        unsafe { core::mem::transmute::<*const dyn ArchMmOps, (usize, usize)>(ptr) };
    ARCH_OPS_DATA.store(data, Ordering::Release);
    ARCH_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的架构操作实现
///
/// # Panics
/// 如果尚未调用 [`register_arch_ops`] 注册实现，则 panic
#[inline]
pub fn arch_ops() -> &'static dyn ArchMmOps {
    let data = ARCH_OPS_DATA.load(Ordering::Acquire);
    let vtable = ARCH_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("mm: ArchMmOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn ArchMmOps>((data, vtable)) }
}
