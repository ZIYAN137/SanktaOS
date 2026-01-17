//! Kernel test assertion recording (non-panicking).
//!
//! In the kernel test environment we do not want `assert!` to panic (panic would abort the whole
//! kernel). Instead, failing assertions are recorded and reported by the custom test runner.

use crate::println;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Copy, Clone, Debug)]
pub struct FailedAssertion {
    pub cond: &'static str,
    pub file: &'static str,
    pub line: u32,
}

impl FailedAssertion {
    pub const fn new(cond: &'static str, file: &'static str, line: u32) -> Self {
        Self { cond, file, line }
    }
}

pub const FAILED_LIST_CAPACITY: usize = 32;

pub static mut FAILED_LIST: [Option<FailedAssertion>; FAILED_LIST_CAPACITY] =
    [None; FAILED_LIST_CAPACITY];
pub static mut FAILED_INDEX: usize = 0;

/// Total number of failed assertions across all tests.
pub static TEST_FAILED: AtomicUsize = AtomicUsize::new(0);

/// Record a failed assertion without panicking.
///
/// This is used as the backend for test-only `assert!*` macros.
pub fn record_failed_assertion(assertion: FailedAssertion) {
    let index = TEST_FAILED.fetch_add(1, Ordering::SeqCst);
    if index < FAILED_LIST_CAPACITY {
        unsafe {
            FAILED_LIST[index] = Some(assertion);
            if index + 1 > FAILED_INDEX {
                FAILED_INDEX = index + 1;
            }
        }
    } else {
        println!(
            "\x1b[91m[warn] Failed assertion list is full (capacity {}). Cannot record: {}\x1b[0m",
            FAILED_LIST_CAPACITY, assertion.cond
        );
    }
}
