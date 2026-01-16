use super::*;

#[test]
fn test_buffer_overflow() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    const TOTAL: usize = 100;
    for i in 0..TOTAL {
        test_log!(log, LogLevel::Info, "log {}", i);
    }

    let buffered = log._log_len();
    let dropped = log._log_dropped_count();

    assert!(dropped > 0);
    assert_eq!(buffered + dropped, TOTAL);
}

#[test]
fn test_overflow_fifo_behavior() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    for i in 0..100 {
        test_log!(log, LogLevel::Info, "entry {}", i);
    }

    let dropped = log._log_dropped_count();
    assert!(dropped > 0);

    let first = log._read_log().unwrap();
    assert!(first.message().starts_with("entry"));
}

#[test]
fn test_write_after_overflow() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    for i in 0..100 {
        test_log!(log, LogLevel::Info, "overflow {}", i);
    }

    let dropped_before = log._log_dropped_count();
    assert!(dropped_before > 0);

    while log._read_log().is_some() {}

    test_log!(log, LogLevel::Info, "after overflow");

    assert_eq!(log._log_len(), 1);
    assert_eq!(log._read_log().unwrap().message(), "after overflow");
}

