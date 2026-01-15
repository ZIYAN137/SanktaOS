//! RTC 设备驱动模块

pub mod rtc_goldfish;

// Re-export device crate 的 RTC 类型
pub use device::rtc::{DateTime, RtcDriver, RTC_DRIVERS};
