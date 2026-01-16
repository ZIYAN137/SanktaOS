//! 网络协议栈模块（re-export from net crate）

pub use net::*;

// 实现 NetOps trait
struct OsNetOps;

impl net::NetOps for OsNetOps {
    fn get_time_ms(&self) -> u64 {
        crate::arch::timer::get_time_ms() as u64
    }

    fn wake_poll_waiters(&self) {
        crate::kernel::syscall::io::wake_poll_waiters();
    }
}

static NET_OPS: OsNetOps = OsNetOps;

/// 初始化网络模块依赖
///
/// 必须在启动早期调用，在使用任何网络功能之前
pub fn init_net_ops() {
    unsafe {
        net::register_net_ops(&NET_OPS);
    }
}
