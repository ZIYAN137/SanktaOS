//! 控制台驱动模块

pub mod frame_console;
pub mod uart_console;

// Re-export device crate 的 Console 类型
pub use device::console::{Console, CONSOLES, MAIN_CONSOLE};

/// 初始化控制台设备
pub fn init() {
    MAIN_CONSOLE.write().replace(CONSOLES.read()[0].clone());
    // frame_console::init();

    // 切换到运行时控制台
    crate::console::init();
    crate::pr_info!("[Console] Switched to runtime console");
}
