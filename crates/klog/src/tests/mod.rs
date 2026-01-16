// Unit tests for klog.
//
// NOTE: These tests used to live under `os/src/log/tests/*` and relied on the kernel test framework.
// They are moved here so `crates/klog` can be validated with standard host `cargo test`.

extern crate alloc;

use crate::log_core::LogCore;
use crate::LogLevel;

/// Test-only logging helper (mirrors production macro behavior, but targets a local `LogCore`).
macro_rules! test_log {
    ($logger:expr, $level:expr, $($arg:tt)*) => {
        $logger._log($level, format_args!($($arg)*))
    };
}

mod basic;
mod byte_counting;
mod filter;
mod format;
mod overflow;
mod peek;

