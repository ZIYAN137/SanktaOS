use super::*;

#[test]
fn test_peek_basic() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "Test message");

    let start = logger._log_reader_index();
    let end = logger._log_writer_index();

    assert_eq!(end, start + 1);

    assert!(logger._peek_log(start).is_some());
    assert_eq!(logger._log_reader_index(), start);

    assert!(logger._peek_log(start).is_some());
    assert_eq!(logger._log_reader_index(), start);
}

#[test]
fn test_peek_vs_read() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");

    let start = logger._log_reader_index();
    let entry_peek = logger._peek_log(start).unwrap();
    assert_eq!(logger._log_reader_index(), start);

    let entry_read = logger._read_log().unwrap();
    assert_eq!(logger._log_reader_index(), start + 1);
    assert_eq!(entry_peek.message(), entry_read.message());
}

#[test]
fn test_peek_multiple() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let start = logger._log_reader_index();

    let e1 = logger._peek_log(start);
    let e2 = logger._peek_log(start + 1);
    let e3 = logger._peek_log(start + 2);

    assert!(e1.is_some());
    assert!(e2.is_some());
    assert!(e3.is_some());

    assert_eq!(logger._log_reader_index(), start);

    assert!(e1.unwrap().message().contains("Message 1"));
    assert!(e2.unwrap().message().contains("Message 2"));
    assert!(e3.unwrap().message().contains("Message 3"));
}

#[test]
fn test_peek_out_of_range() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    test_log!(logger, LogLevel::Info, "Test");

    let start = logger._log_reader_index();
    let end = logger._log_writer_index();

    assert!(logger._peek_log(start).is_some());
    assert!(logger._peek_log(end).is_none());
    assert!(logger._peek_log(end + 1).is_none());
    assert!(logger._peek_log(start - 1).is_none());
}

#[test]
fn test_peek_after_read() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    test_log!(logger, LogLevel::Info, "Message 1");
    test_log!(logger, LogLevel::Info, "Message 2");
    test_log!(logger, LogLevel::Info, "Message 3");

    let start = logger._log_reader_index();
    logger._read_log();

    assert!(logger._peek_log(start).is_none());
    assert!(logger._peek_log(start + 1).is_some());
    assert!(logger._peek_log(start + 2).is_some());
}

#[test]
fn test_peek_index_tracking() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    let r0 = logger._log_reader_index();
    let w0 = logger._log_writer_index();
    assert_eq!(r0, w0);

    test_log!(logger, LogLevel::Info, "Test");
    assert_eq!(logger._log_writer_index(), w0 + 1);
    assert_eq!(logger._log_reader_index(), r0);

    logger._read_log();
    assert_eq!(logger._log_reader_index(), r0 + 1);
    assert_eq!(logger._log_reader_index(), logger._log_writer_index());
}

#[test]
fn test_peek_with_byte_counting() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    test_log!(logger, LogLevel::Info, "Test");

    let bytes_before = logger._log_unread_bytes();
    let start = logger._log_reader_index();

    logger._peek_log(start);
    assert_eq!(logger._log_unread_bytes(), bytes_before);

    logger._peek_log(start);
    assert_eq!(logger._log_unread_bytes(), bytes_before);

    logger._read_log();
    assert_eq!(logger._log_unread_bytes(), 0);
}

#[test]
fn test_peek_empty_buffer() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);
    let start = logger._log_reader_index();
    assert!(logger._peek_log(start).is_none());
    assert!(logger._peek_log(start + 1).is_none());
}

#[test]
fn test_peek_sequential_access() {
    let logger = LogCore::new(LogLevel::Debug, LogLevel::Emergency);

    for i in 0..5 {
        test_log!(logger, LogLevel::Info, "Message {}", i);
    }

    let start = logger._log_reader_index();
    for i in 0..5 {
        assert!(logger._peek_log(start + i).is_some());
    }
    assert!(logger._peek_log(start + 5).is_none());
    assert_eq!(logger._log_reader_index(), start);
}

