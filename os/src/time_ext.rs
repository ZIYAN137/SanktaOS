//! TimeSpec 扩展方法（依赖 arch/kernel）
//!
//! 这些方法需要访问架构相关的定时器和内核时间状态，
//! 因此无法放在独立的 uapi crate 中。

use uapi::time::TimeSpec;

use crate::arch::timer::{clock_freq, get_time};

/// 获取当前墙上时钟时间
///
/// 返回自 Unix 纪元以来的时间（CLOCK_REALTIME）
pub fn timespec_now() -> TimeSpec {
    // 使用 kernel::time::realtime_now() 避免循环依赖
    crate::kernel::time::realtime_now()
}

/// 获取当前单调时钟时间
///
/// 返回自系统启动以来的时间（CLOCK_MONOTONIC）
pub fn timespec_monotonic_now() -> TimeSpec {
    let time = get_time();
    TimeSpec::from_freq(time, clock_freq())
}
