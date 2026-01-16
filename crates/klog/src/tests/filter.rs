use super::*;

#[test]
fn test_global_level_filtering() {
    let log = LogCore::new(LogLevel::Warning, LogLevel::Warning);

    test_log!(log, LogLevel::Emergency, "emergency");
    test_log!(log, LogLevel::Error, "error");
    test_log!(log, LogLevel::Warning, "warning");
    test_log!(log, LogLevel::Info, "info");
    test_log!(log, LogLevel::Debug, "debug");

    assert_eq!(log._log_len(), 3);
    assert_eq!(log._read_log().unwrap().message(), "emergency");
    assert_eq!(log._read_log().unwrap().message(), "error");
    assert_eq!(log._read_log().unwrap().message(), "warning");
    assert_eq!(log._log_len(), 0);
}

#[test]
fn test_level_boundary() {
    let log = LogCore::new(LogLevel::Info, LogLevel::Warning);

    test_log!(log, LogLevel::Info, "boundary");
    assert_eq!(log._log_len(), 1);

    test_log!(log, LogLevel::Debug, "filtered");
    assert_eq!(log._log_len(), 1);

    assert_eq!(log._read_log().unwrap().message(), "boundary");
}

#[test]
fn test_dynamic_level_change() {
    let log = LogCore::new(LogLevel::Info, LogLevel::Warning);

    test_log!(log, LogLevel::Debug, "debug1");
    test_log!(log, LogLevel::Info, "info1");

    assert_eq!(log._log_len(), 1);

    log._set_global_level(LogLevel::Debug);

    test_log!(log, LogLevel::Debug, "debug2");
    test_log!(log, LogLevel::Info, "info2");

    assert_eq!(log._log_len(), 3);
    assert_eq!(log._read_log().unwrap().message(), "info1");
    assert_eq!(log._read_log().unwrap().message(), "debug2");
    assert_eq!(log._read_log().unwrap().message(), "info2");
}

#[test]
fn test_all_levels() {
    let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

    test_log!(log, LogLevel::Emergency, "emerg");
    test_log!(log, LogLevel::Alert, "alert");
    test_log!(log, LogLevel::Critical, "crit");
    test_log!(log, LogLevel::Error, "err");
    test_log!(log, LogLevel::Warning, "warn");
    test_log!(log, LogLevel::Notice, "notice");
    test_log!(log, LogLevel::Info, "info");
    test_log!(log, LogLevel::Debug, "debug");

    assert_eq!(log._log_len(), 8);
    for expected in [
        "emerg", "alert", "crit", "err", "warn", "notice", "info", "debug",
    ] {
        assert_eq!(log._read_log().unwrap().message(), expected);
    }
}

