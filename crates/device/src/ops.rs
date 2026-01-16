//! 中断操作 trait 定义和注册
//!
//! 此模块定义了 device crate 需要的中断操作接口，通过 trait 抽象实现与 os crate 的解耦。

use core::sync::atomic::{AtomicUsize, Ordering};

/// 中断操作
///
/// 此 trait 抽象了中断相关的操作。
/// os crate 需要实现此 trait 并在启动时注册。
pub trait IrqOps: Send + Sync {
    /// 启用指定中断号
    fn enable_irq(&self, irq: usize);
}

static IRQ_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static IRQ_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册中断操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_irq_ops(ops: &'static dyn IrqOps) {
    let ptr = ops as *const dyn IrqOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) = unsafe { core::mem::transmute::<*const dyn IrqOps, (usize, usize)>(ptr) };
    IRQ_OPS_DATA.store(data, Ordering::Release);
    IRQ_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的中断操作实现
///
/// # Panics
/// 如果尚未调用 [`register_irq_ops`] 注册实现，则 panic
#[inline]
pub fn irq_ops() -> &'static dyn IrqOps {
    let data = IRQ_OPS_DATA.load(Ordering::Acquire);
    let vtable = IRQ_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        panic!("device: IrqOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn IrqOps>((data, vtable)) }
}
