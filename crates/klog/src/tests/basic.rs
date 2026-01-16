use super::*;

#[test]
fn test_write_and_read() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "test message");

    assert_eq!(log._log_len(), 1);

    let entry = log._read_log().unwrap();
    assert_eq!(entry.message(), "test message");
    assert_eq!(entry.level(), LogLevel::Info);

    assert_eq!(log._log_len(), 0);
}

#[test]
fn test_format_arguments() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "value: {}", 42);
    test_log!(log, LogLevel::Debug, "hex: {:#x}", 0xDEAD);

    let e1 = log._read_log().unwrap();
    assert_eq!(e1.message(), "value: 42");

    let e2 = log._read_log().unwrap();
    assert_eq!(e2.message(), "hex: 0xdead");
}

#[test]
fn test_fifo_order() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    for i in 0..5 {
        test_log!(log, LogLevel::Debug, "message {}", i);
    }

    assert_eq!(log._log_len(), 5);

    for i in 0..5 {
        let entry = log._read_log().unwrap();
        let expected = alloc::format!("message {}", i);
        assert_eq!(entry.message(), expected.as_str());
    }

    assert_eq!(log._log_len(), 0);
}

#[test]
fn test_empty_buffer_read() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    assert_eq!(log._log_len(), 0);
    assert!(log._read_log().is_none());
    assert!(log._read_log().is_none());
    assert!(log._read_log().is_none());
}

