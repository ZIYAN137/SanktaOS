//! 网络运行时操作 trait 定义和注册
//!
//! 此模块定义了网络层需要的外部依赖接口，通过 trait 抽象实现与 os crate 的解耦。

use core::sync::atomic::{AtomicUsize, Ordering};

/// 网络运行时操作
///
/// 此 trait 抽象了网络层需要的运行时操作，包括时间获取和任务唤醒。
/// os crate 需要实现此 trait 并在启动时注册。
pub trait NetOps: Send + Sync {
    /// 获取当前时间戳（毫秒）
    ///
    /// 用于 smoltcp 协议栈的时间戳计算
    fn get_time_ms(&self) -> u64;

    /// 唤醒等待 poll 的任务
    ///
    /// 当网络事件发生时（如数据到达、连接建立等），调用此方法唤醒等待的任务
    fn wake_poll_waiters(&self);
}

// 使用 AtomicUsize 存储 fat pointer 的两部分
static NET_OPS_DATA: AtomicUsize = AtomicUsize::new(0);
static NET_OPS_VTABLE: AtomicUsize = AtomicUsize::new(0);

/// 注册网络操作实现
///
/// # Safety
/// 必须在单线程环境下调用，且只能调用一次
pub unsafe fn register_net_ops(ops: &'static dyn NetOps) {
    let ptr = ops as *const dyn NetOps;
    // SAFETY: 将 fat pointer 拆分为 data 和 vtable 两部分存储
    let (data, vtable) = unsafe { core::mem::transmute::<*const dyn NetOps, (usize, usize)>(ptr) };
    NET_OPS_DATA.store(data, Ordering::Release);
    NET_OPS_VTABLE.store(vtable, Ordering::Release);
}

/// 获取已注册的网络操作实现
///
/// # Panics
/// 如果尚未调用 [`register_net_ops`] 注册实现，则 panic
#[inline]
pub fn net_ops() -> &'static dyn NetOps {
    let data = NET_OPS_DATA.load(Ordering::Acquire);
    let vtable = NET_OPS_VTABLE.load(Ordering::Acquire);
    if data == 0 {
        #[cfg(test)]
        {
            extern crate test_support;
            return &test_support::mock::net::MOCK_NET_OPS;
        }
        #[cfg(not(test))]
        panic!("net: NetOps not registered");
    }
    // SAFETY: 重组 fat pointer
    unsafe { &*core::mem::transmute::<(usize, usize), *const dyn NetOps>((data, vtable)) }
}

#[cfg(test)]
mod test_mock {
    extern crate test_support;

    use super::NetOps;

    impl NetOps for test_support::mock::net::MockNetOps {
        fn get_time_ms(&self) -> u64 {
            0
        }

        fn wake_poll_waiters(&self) {}
    }

    #[test]
    fn test_net_ops_fallback_does_not_panic() {
        assert_eq!(super::net_ops().get_time_ms(), 0);
        super::net_ops().wake_poll_waiters();
    }
}
