//! 日志系统核心实现
//!
//! 该模块将所有日志状态和逻辑封装到一个单独的 `LogCore` 结构体中，
//! 可以在保持**无锁、零分配**设计的同时，独立实例化用于测试。

use super::buffer::GlobalLogBuffer;
use super::config::{DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL};
use super::entry::LogEntry;
use super::level::LogLevel;
use core::fmt;
use core::sync::atomic::{AtomicU8, Ordering};

/// 核心日志系统
///
/// 封装了环形缓冲区和过滤状态。可以为测试目的而实例化，
/// 或在生产环境中用作全局单例。
///
/// # 线程安全性
///
/// 所有方法都使用原子操作进行同步，使得整个结构体在
/// 线程之间安全共享，无需外部加锁。
pub struct LogCore {
    /// 用于日志存储的无锁环形缓冲区
    buffer: GlobalLogBuffer,

    /// 全局日志级别阈值（控制日志是否缓冲）
    global_level: AtomicU8,

    /// 控制台输出级别阈值（控制是否立即打印）
    console_level: AtomicU8,
}

impl LogCore {
    /// 使用默认日志级别创建新的 LogCore 实例
    ///
    /// 这是一个 `const fn`，可以在编译时进行评估，
    /// 从而实现零开销的静态初始化。
    ///
    /// 使用配置中的默认级别：
    /// - 全局级别: Info (Debug 级别的日志将被过滤)
    /// - 控制台级别: Warning (只打印 Warning 和 Error 级别的日志)
    ///
    /// # 示例
    ///
    /// ```rust
    /// use klog::LogCore;
    ///
    /// // Global singleton (const-initialized)
    /// static GLOBAL_LOG: LogCore = LogCore::default();
    /// let _ = &GLOBAL_LOG;
    /// ```
    pub const fn default() -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(DEFAULT_LOG_LEVEL as u8),
            console_level: AtomicU8::new(DEFAULT_CONSOLE_LEVEL as u8),
        }
    }

    /// 使用自定义日志级别创建新的 LogCore 实例
    ///
    /// 此构造函数允许在创建时指定全局和控制台日志级别，
    /// 这对于测试尤其有用。
    ///
    /// # 参数
    ///
    /// * `global_level` - 日志被缓冲的最低级别
    /// * `console_level` - 日志被打印到控制台的最低级别
    ///
    /// # 示例
    ///
    /// ```rust
    /// use klog::{LogCore, LogLevel};
    ///
    /// // Test instance (enable Debug)
    /// let _test_log = LogCore::new(LogLevel::Debug, LogLevel::Warning);
    ///
    /// // Production instance (custom levels)
    /// let _log = LogCore::new(LogLevel::Info, LogLevel::Error);
    /// ```
    pub fn new(global_level: LogLevel, console_level: LogLevel) -> Self {
        Self {
            buffer: GlobalLogBuffer::new(),
            global_level: AtomicU8::new(global_level as u8),
            console_level: AtomicU8::new(console_level as u8),
        }
    }

    /// 核心日志记录实现
    ///
    /// 此方法由生产宏（通过 GLOBAL_LOG）和测试代码（通过本地实例）调用。
    ///
    /// # 无锁操作
    ///
    /// 1. 原子读取 global_level (Acquire)
    /// 2. 如果被过滤，则提前返回
    /// 3. 收集上下文 (时间戳、CPU ID、任务 ID)
    /// 4. 创建日志条目 (栈分配)
    /// 5. 原子缓冲区写入 (无锁)
    /// 6. 可选的控制台输出 (如果满足 console_level)
    ///
    /// # 参数
    ///
    /// * `level` - 日志级别 (Emergency 到 Debug)
    /// * `args` - 来自 `format_args!` 的格式化参数
    pub fn _log(&self, level: LogLevel, args: fmt::Arguments) {
        // 1. 早期过滤 (全局级别)
        if !self.is_level_enabled(level) {
            return;
        }

        // 2. 收集上下文（通过 trait）
        let (cpu_id, task_id, timestamp) = if let Some(provider) = crate::get_context_provider() {
            (provider.cpu_id(), provider.task_id(), provider.timestamp())
        } else {
            // 如果没有注册 provider，使用默认值
            (0, 0, 0)
        };

        // 3. 创建日志条目
        let entry = LogEntry::from_args(level, cpu_id, task_id, timestamp, args);

        // 4. 写入缓冲区 (无锁)
        self.buffer.write(&entry);

        // 5. 可选的即时控制台输出
        if self.is_console_level(level) {
            self.direct_print_entry(&entry);
        }
    }

    /// 从缓冲区读取下一个日志条目
    ///
    /// 如果没有可用条目，则返回 `None`。这是一个**无锁**的
    /// 单消费者操作。
    pub fn _read_log(&self) -> Option<LogEntry> {
        self.buffer.read()
    }

    /// 非破坏性读取：按索引 peek 日志条目，不移动读指针
    pub fn _peek_log(&self, index: usize) -> Option<LogEntry> {
        self.buffer.peek(index)
    }

    /// 获取当前可读取的起始索引
    pub fn _log_reader_index(&self) -> usize {
        self.buffer.reader_index()
    }

    /// 获取当前写入位置
    pub fn _log_writer_index(&self) -> usize {
        self.buffer.writer_index()
    }

    /// 返回未读日志条目的数量
    pub fn _log_len(&self) -> usize {
        self.buffer.len()
    }

    /// 返回未读日志的总字节数（格式化后）
    pub fn _log_unread_bytes(&self) -> usize {
        self.buffer.unread_bytes()
    }

    /// 返回由于缓冲区溢出而丢弃的日志计数
    pub fn _log_dropped_count(&self) -> usize {
        self.buffer.dropped_count()
    }

    /// 设置全局日志级别阈值
    ///
    /// 级别 > 阈值的日志将被丢弃。
    ///
    /// # 内存顺序
    ///
    /// 使用 Release 顺序以确保新级别对所有核心可见。
    pub fn _set_global_level(&self, level: LogLevel) {
        self.global_level.store(level as u8, Ordering::Release);
    }

    /// 获取当前全局日志级别
    pub fn _get_global_level(&self) -> LogLevel {
        let level = self.global_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    /// 设置控制台输出级别阈值
    ///
    /// 只有级别 <= 阈值的日志才会立即打印。
    pub fn _set_console_level(&self, level: LogLevel) {
        self.console_level.store(level as u8, Ordering::Release);
    }

    /// 获取当前控制台输出级别
    pub fn _get_console_level(&self) -> LogLevel {
        let level = self.console_level.load(Ordering::Acquire);
        LogLevel::from_u8(level)
    }

    // ========== 内部辅助函数 ==========

    /// 检查日志级别是否启用 (全局过滤器)
    #[inline(always)]
    fn is_level_enabled(&self, level: LogLevel) -> bool {
        level as u8 <= self.global_level.load(Ordering::Acquire)
    }

    /// 检查日志是否应该打印到控制台
    #[inline(always)]
    fn is_console_level(&self, level: LogLevel) -> bool {
        level as u8 <= self.console_level.load(Ordering::Acquire)
    }

    /// 使用 ANSI 颜色直接将日志条目打印到控制台（无堆分配）
    ///
    /// 此方法在早期启动时即可使用，因为它仅使用栈和 core::fmt::Write，
    /// 不依赖堆分配器。
    ///
    /// **重要**: 此函数的格式化逻辑必须与 `format_log_entry` 和 `buffer::calculate_formatted_length` 保持一致。
    /// 如果修改了日志输出格式，需要同步更新三处：
    /// - `direct_print_entry` (此函数) - 用于早期启动的控制台输出
    /// - `format_log_entry` - 用于 syslog 系统调用
    /// - `buffer::calculate_formatted_length` - 用于精确字节计数
    fn direct_print_entry(&self, entry: &LogEntry) {
        use alloc::format;

        // 格式化日志条目
        let formatted = format!(
            "{}{} [{:12}] [CPU{}/T{:3}] {}{}\n",
            entry.level().color_code(),
            entry.level().as_str(),
            entry.timestamp(),
            entry.cpu_id(),
            entry.task_id(),
            entry.message(),
            entry.level().reset_color_code()
        );

        // 通过 trait 输出
        if let Some(output) = crate::get_log_output() {
            output.write_str(&formatted);
        }
    }
}

// 标记为 Sync 允许在 static 中使用
unsafe impl Sync for LogCore {}

/// 格式化日志条目为字符串（带 ANSI 颜色和上下文信息）
///
/// 将 LogEntry 格式化为用户可读的字符串，用于 syslog 系统调用等场景。
/// 包含 ANSI 颜色代码、时间戳、CPU ID、任务 ID 等上下文信息。
///
/// **注意**：此函数使用堆分配（`alloc::format!`），仅在堆分配器初始化后可用。
/// 主要用于 syslog 系统调用等运行时场景。早期启动时的控制台输出使用
/// `direct_print_entry` 方法，该方法不依赖堆分配。
///
/// **重要**：此函数的格式化逻辑必须与 `direct_print_entry` 和 `buffer::calculate_formatted_length` 保持一致。
/// 如果修改了日志输出格式，需要同步更新三处：
/// - `direct_print_entry` - 用于早期启动的控制台输出（无堆分配）
/// - `format_log_entry` (此函数) - 用于 syslog 系统调用（使用堆分配）
/// - `buffer::calculate_formatted_length` - 用于精确字节计数
///
/// # 格式
/// ```text
/// <color_code>[LEVEL] [timestamp] [CPU<id>/T<tid>] message<reset>
/// ```
///
/// # 示例
/// ```text
/// \x1b[37m[INFO] [      123456] [CPU0/T  1] Kernel initialized\x1b[0m
/// \x1b[31m[ERR] [      789012] [CPU0/T  5] Failed to mount /dev/sda1\x1b[0m
/// ```
///
/// # 参数
/// * `entry` - 要格式化的日志条目
///
/// # 返回值
/// 格式化后的字符串（包含 ANSI 颜色代码和上下文信息）
pub fn format_log_entry(entry: &LogEntry) -> alloc::string::String {
    use alloc::format;

    format!(
        "{}{} [{:12}] [CPU{}/T{:3}] {}{}",
        entry.level().color_code(),
        entry.level().as_str(),
        entry.timestamp(),
        entry.cpu_id(),
        entry.task_id(),
        entry.message(),
        entry.level().reset_color_code()
    )
}

#[cfg(test)]
mod tests {
    // Unit tests for LogCore.
    //
    // NOTE: These tests used to live under `os/src/log/tests/*` and relied on the kernel test
    // framework. They are moved here so `crates/klog` can be validated with standard host
    // `cargo test`, while still testing internal (non-public) LogCore behavior.
    extern crate alloc;

    use super::*;

    /// Test-only logging helper (mirrors production macro behavior, but targets a local `LogCore`).
    macro_rules! test_log {
        ($logger:expr, $level:expr, $($arg:tt)*) => {
            $logger._log($level, format_args!($($arg)*))
        };
    }

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

    #[test]
    fn test_message_truncation() {
        let log = LogCore::new(LogLevel::Debug, LogLevel::Warning);

        // Create a long message (> MAX_LOG_MESSAGE_LENGTH).
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
}
