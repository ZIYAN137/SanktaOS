//! 内存管理配置 trait 定义和注册

use core::sync::atomic::{AtomicUsize, Ordering};

/// 内存管理配置常量
///
/// 此 trait 提供内存管理所需的配置常量。
/// os crate 需要实现此 trait 并注册。
pub trait MmConfig: Send + Sync {
    /// 页大小（通常为 4096）
    fn page_size(&self) -> usize;

    /// 物理内存结束地址
    fn memory_end(&self) -> usize;

    /// 用户栈大小
    fn user_stack_size(&self) -> usize;

    /// 用户栈顶地址
    fn user_stack_top(&self) -> usize;

    /// 最大用户堆大小
    fn max_user_heap_size(&self) -> usize;

    /// 信号返回跳板地址
    fn user_sigreturn_trampoline(&self) -> usize;
}

static CONFIG_DATA: AtomicUsize = AtomicUsize::new(0);
static CONFIG_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册配置实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_config(config: &'static dyn MmConfig) {
    let ptr = config as *const dyn MmConfig;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) =
        unsafe { core::mem::transmute::<*const dyn MmConfig, (usize, usize)>(ptr) };
    CONFIG_DATA.store(data, Ordering::Release);
    CONFIG_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的配置实现
///
/// # Panics
/// 如果尚未调用 [`register_config`] 注册实现，则 panic
#[inline]
pub fn mm_config() -> &'static dyn MmConfig {
    let data = CONFIG_DATA.load(Ordering::Acquire);
    let vtable = CONFIG_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("mm: MmConfig not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn MmConfig>((data, vtable)) }
}
