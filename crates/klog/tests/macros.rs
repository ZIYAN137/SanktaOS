//! Integration tests for klog public API (macros + global registration).

use std::sync::{Mutex, Once, OnceLock};

use klog::{pr_debug, pr_err, pr_info, pr_warn, LogContextProvider, LogLevel, LogOutput};

static INIT: Once = Once::new();

static OUTPUT_BUF: OnceLock<Mutex<String>> = OnceLock::new();

struct TestOutput;

impl LogOutput for TestOutput {
    fn write_str(&self, s: &str) {
        let buf = OUTPUT_BUF.get_or_init(|| Mutex::new(String::new()));
        buf.lock().unwrap().push_str(s);
    }
}

static TEST_OUTPUT: TestOutput = TestOutput;

struct TestContextProvider;

impl LogContextProvider for TestContextProvider {
    fn cpu_id(&self) -> usize {
        1
    }

    fn task_id(&self) -> u32 {
        42
    }

    fn timestamp(&self) -> usize {
        123456
    }
}

static TEST_PROVIDER: TestContextProvider = TestContextProvider;

fn init_once() {
    INIT.call_once(|| unsafe {
        klog::register_log_output(&TEST_OUTPUT);
        klog::register_context_provider(&TEST_PROVIDER);
    });
}

fn drain_logs() {
    while klog::read_log().is_some() {}
}

fn take_output() -> String {
    let buf = OUTPUT_BUF.get_or_init(|| Mutex::new(String::new()));
    let mut g = buf.lock().unwrap();
    let out = g.clone();
    g.clear();
    out
}

#[test]
fn test_pr_info_buffered_and_console_by_default() {
    init_once();
    drain_logs();
    take_output();

    pr_info!("hello {}", 1);

    assert_eq!(klog::log_len(), 1);
    let entry = klog::read_log().unwrap();
    assert_eq!(entry.level(), LogLevel::Info);
    assert_eq!(entry.message(), "hello 1");

    // Current default console threshold is Info, so Info is printed.
    let out = take_output();
    assert!(out.contains("hello 1"));
}

#[test]
fn test_pr_err_prints_to_console() {
    init_once();
    drain_logs();
    take_output();

    pr_err!("boom: {}", "EIO");

    // pr_err is console-visible at default threshold.
    let out = take_output();
    assert!(out.contains("boom: EIO"));

    // It should also be buffered.
    let entry = klog::read_log().unwrap();
    assert_eq!(entry.level(), LogLevel::Error);
    assert_eq!(entry.message(), "boom: EIO");
}

#[test]
fn test_context_provider_applied_to_entries() {
    init_once();
    drain_logs();

    pr_warn!("ctx");

    let entry = klog::read_log().unwrap();
    assert_eq!(entry.cpu_id(), 1);
    assert_eq!(entry.task_id(), 42);
    assert_eq!(entry.timestamp(), 123456);
}

#[test]
fn test_pr_debug_filtered_by_default_level() {
    init_once();
    drain_logs();
    take_output();

    pr_debug!("should not be logged");

    assert_eq!(klog::log_len(), 0);
    assert_eq!(take_output(), "");
}
