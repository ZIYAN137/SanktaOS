use super::*;

#[test]
fn test_message_truncation() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Create a long message (> MAX_LOG_MESSAGE_LENGTH)
    let long_msg = "a".repeat(300);
    test_log!(log, LogLevel::Info, "{}", long_msg);

    let entry = log._read_log().unwrap();
    assert!(entry.message().len() <= crate::MAX_LOG_MESSAGE_LENGTH);
}

#[test]
fn test_empty_message() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);
    test_log!(log, LogLevel::Info, "");
    let entry = log._read_log().unwrap();
    assert_eq!(entry.message(), "");
}

#[test]
fn test_special_characters() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);
    test_log!(log, LogLevel::Info, "special: !@#$%^&*()");
    let entry = log._read_log().unwrap();
    assert_eq!(entry.message(), "special: !@#$%^&*()");
}

#[test]
fn test_utf8_message() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    // Non-ASCII strings are intentional here to validate UTF-8 handling.
    test_log!(log, LogLevel::Info, "你好，世界！");
    test_log!(log, LogLevel::Info, "Hello, мир!");

    let e1 = log._read_log().unwrap();
    assert_eq!(e1.message(), "你好，世界！");

    let e2 = log._read_log().unwrap();
    assert_eq!(e2.message(), "Hello, мир!");
}
