// Tests for precise unread-bytes tracking.

use super::*;

#[test]
fn test_unread_bytes_basic() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    assert_eq!(logger._log_unread_bytes(), 0);

    test_log!(logger, LogLevel::Info, "Test message");

    let after_write = logger._log_unread_bytes();
    assert!(after_write > 0);

    let _ = logger._read_log();

    let after_read = logger._log_unread_bytes();
    assert!(after_read < after_write);
    assert_eq!(after_read, 0);
}

#[test]
fn test_unread_bytes_multiple() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let total = logger._log_unread_bytes();
    assert!(total > 0);

    let _ = logger._read_log();
    let after_one = logger._log_unread_bytes();
    assert!(after_one < total);
    assert!(after_one > 0);

    let _ = logger._read_log();
    let after_two = logger._log_unread_bytes();
    assert!(after_two < after_one);
    assert!(after_two > 0);

    let _ = logger._read_log();
    let after_three = logger._log_unread_bytes();
    assert_eq!(after_three, 0);
}

#[test]
fn test_unread_bytes_accuracy() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    test_log!(logger, LogLevel::Info, "Hello");

    let reported = logger._log_unread_bytes();
    let entry = logger._read_log().unwrap();
    let formatted = crate::format_log_entry(&entry);
    let actual = formatted.len();

    // Context fields (cpu/task/timestamp) may vary; just check a reasonable bound.
    assert!(reported > 0);
    assert!(reported >= actual.saturating_sub(10));
    assert!(reported <= actual + 10);
}

#[test]
fn test_unread_bytes_different_lengths() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "A");
    let bytes_short = logger._log_unread_bytes();

    test_log!(
        logger,
        LogLevel::Info,
        "This is a much longer message with more content"
    );
    let bytes_both = logger._log_unread_bytes();

    assert!(bytes_both > bytes_short);
    let diff = bytes_both - bytes_short;
    assert!(diff > 30);
}

#[test]
fn test_unread_bytes_with_different_levels() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Emergency, "Test");
    let bytes_emerg = logger._log_unread_bytes();
    let _ = logger._read_log();

    test_log!(logger, LogLevel::Info, "Test");
    let bytes_info = logger._log_unread_bytes();

    assert_ne!(bytes_emerg, bytes_info);
}

#[test]
fn test_unread_bytes_empty_message() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    test_log!(logger, LogLevel::Info, "");

    let bytes = logger._log_unread_bytes();
    assert!(bytes > 40);
}

#[test]
fn test_unread_bytes_persistence() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    test_log!(logger, LogLevel::Info, "Persistent");

    let initial = logger._log_unread_bytes();
    assert_eq!(logger._log_unread_bytes(), initial);
    assert_eq!(logger._log_unread_bytes(), initial);
    assert_eq!(logger._log_unread_bytes(), initial);

    let _ = logger._read_log();
    assert_eq!(logger._log_unread_bytes(), 0);
}

