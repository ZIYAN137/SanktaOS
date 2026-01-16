//! ComixOS - A RISC-V operating system kernel
//!
//! This is the main crate for ComixOS, an operating system kernel written in Rust
//! for RISC-V architecture. It provides basic OS functionalities including memory
//! management, process scheduling, and system call handling.

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;
extern crate uapi;

// ========== Kernel Test Assertions ==========
//
// In `cfg(test)` builds, map `assert!*` to the kernel test assertion recorder instead of panicking.
// Keep production assertions explicit via `core::assert!` where needed.
#[cfg(test)]
#[macro_export]
/// Test-only `assert!` that records a failure instead of panicking.
macro_rules! assert {
    ($cond:expr $(,)?) => {{
        if !$cond {
            let fa =
                $crate::test::assert::FailedAssertion::new(stringify!($cond), file!(), line!());
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
    ($cond:expr, $($arg:tt)+) => {{
        if !$cond {
            let fa =
                $crate::test::assert::FailedAssertion::new(stringify!($cond), file!(), line!());
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
}

#[cfg(test)]
#[macro_export]
/// Test-only `assert_eq!` that records a failure instead of panicking.
macro_rules! assert_eq {
    ($left:expr, $right:expr $(,)?) => {{
        let left = &$left;
        let right = &$right;
        if !(left == right) {
            let fa = $crate::test::assert::FailedAssertion::new(
                stringify!($left == $right),
                file!(),
                line!(),
            );
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left = &$left;
        let right = &$right;
        if !(left == right) {
            let fa = $crate::test::assert::FailedAssertion::new(
                stringify!($left == $right),
                file!(),
                line!(),
            );
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
}

#[cfg(test)]
#[macro_export]
/// Test-only `assert_ne!` that records a failure instead of panicking.
macro_rules! assert_ne {
    ($left:expr, $right:expr $(,)?) => {{
        let left = &$left;
        let right = &$right;
        if !(left != right) {
            let fa = $crate::test::assert::FailedAssertion::new(
                stringify!($left != $right),
                file!(),
                line!(),
            );
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left = &$left;
        let right = &$right;
        if !(left != right) {
            let fa = $crate::test::assert::FailedAssertion::new(
                stringify!($left != $right),
                file!(),
                line!(),
            );
            $crate::test::assert::record_failed_assertion(fa);
        }
    }};
}

mod arch;
mod config;
mod console;
mod device;
mod fs;
mod ipc;
mod kernel;
mod mm;
mod security;
mod sync;
mod test;
pub mod time_ext;
mod util;
mod vfs;
#[macro_use]
mod log;
mod net;

use crate::arch::lib::sbi::shutdown;
#[cfg(target_arch = "loongarch64")]
use core::arch::asm;
use core::panic::PanicInfo;
#[cfg(test)]
use test::test_runner;

/// Rust 内核主入口点
///
/// 这是从汇编代码跳转到的第一个 Rust 函数。它负责初始化内核的所有子系统,
/// 包括内存管理、中断处理、定时器和任务调度器。
///
/// # Safety
///
/// 此函数标记为 `#[unsafe(no_mangle)]` 以确保链接器可以找到它。
/// 它必须从正确初始化的汇编入口点调用。
#[unsafe(no_mangle)]
pub extern "C" fn rust_main(_hartid: usize) -> ! {
    arch::boot::main(_hartid);
    unreachable!("Unreachable in rust_main()");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        earlyprintln!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message()
        );
    } else {
        earlyprintln!("Panicked: {}", info.message());
    }

    shutdown(true)
}
