//! 内核日志子系统
//!
//! 该模块重新导出 klog crate 的功能，并提供架构特定的实现。

#![allow(unused)]

// 重新导出 klog crate 的所有公共 API
pub use klog::{
    format_log_entry, get_console_level, get_global_level, is_level_enabled, log_dropped_count,
    log_impl, log_len, log_reader_index, log_unread_bytes, log_writer_index, peek_log, read_log,
    set_console_level, set_global_level, LogContextProvider, LogEntry, LogLevel, LogOutput,
    DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL, GLOBAL_LOG_BUFFER_SIZE, MAX_LOG_MESSAGE_LENGTH,
};

use crate::arch::{kernel::cpu::cpu_id, timer};
use crate::console::Stdout;
use crate::sync::PreemptGuard;
use core::fmt::Write;

// ========== 重新定义宏以支持 crate:: 前缀 ==========

/// 带有级别过滤的内部实现宏
#[macro_export]
macro_rules! __log_impl_filtered {
    ($level:expr, $args:expr) => {
        if $crate::log::is_level_enabled($level) {
            $crate::log::log_impl($level, $args);
        }
    };
}

/// 以 **EMERGENCY (紧急)** 级别记录消息
#[macro_export]
macro_rules! pr_emerg {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Emergency,
            format_args!($($arg)*)
        )
    }
}

/// 以 **ALERT (警报)** 级别记录消息
#[macro_export]
macro_rules! pr_alert {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Alert,
            format_args!($($arg)*)
        )
    }
}

/// 以 **CRITICAL (关键)** 级别记录消息
#[macro_export]
macro_rules! pr_crit {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Critical,
            format_args!($($arg)*)
        )
    }
}

/// 以 **ERROR (错误)** 级别记录消息
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Error,
            format_args!($($arg)*)
        )
    }
}

/// 以 **WARNING (警告)** 级别记录消息
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Warning,
            format_args!($($arg)*)
        )
    }
}

/// 以 **NOTICE (通知)** 级别记录消息
#[macro_export]
macro_rules! pr_notice {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Notice,
            format_args!($($arg)*)
        )
    }
}

/// 以 **INFO (信息)** 级别记录消息
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Info,
            format_args!($($arg)*)
        )
    }
}

/// 以 **DEBUG (调试)** 级别记录消息
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        $crate::__log_impl_filtered!(
            $crate::log::LogLevel::Debug,
            format_args!($($arg)*)
        )
    }
}

// ========== LogContextProvider 实现 ==========

/// OS 层的日志上下文提供者
struct OsLogContextProvider;

impl LogContextProvider for OsLogContextProvider {
    fn cpu_id(&self) -> usize {
        cpu_id()
    }

    fn task_id(&self) -> u32 {
        // 尝试获取当前任务的 tid
        // 注意：在早期启动或中断上下文中可能没有当前任务
        // 更重要的是：如果当前已经在持有 task lock 的上下文中（例如 wait4），
        // 再次尝试获取锁会导致死锁。因此这里必须使用 try_lock。
        let _guard = PreemptGuard::new();
        crate::kernel::current_cpu()
            .current_task
            .as_ref()
            .and_then(|task| task.try_lock().map(|t| t.tid))
            .unwrap_or(0)
    }

    fn timestamp(&self) -> usize {
        timer::get_time()
    }
}

// ========== LogOutput 实现 ==========

/// OS 层的日志输出
struct OsLogOutput;

impl LogOutput for OsLogOutput {
    fn write_str(&self, s: &str) {
        let mut stdout = Stdout;
        let _ = stdout.write_str(s);
    }
}

// ========== 全局实例 ==========

static OS_LOG_CONTEXT_PROVIDER: OsLogContextProvider = OsLogContextProvider;
static OS_LOG_OUTPUT: OsLogOutput = OsLogOutput;

/// 初始化日志系统
///
/// 注册 OS 层的 LogContextProvider 和 LogOutput 实现。
/// 必须在使用日志宏之前调用。
pub fn init() {
    // Safety: 这些是静态实例，生命周期为 'static
    unsafe {
        klog::register_context_provider(&OS_LOG_CONTEXT_PROVIDER);
        klog::register_log_output(&OS_LOG_OUTPUT);
    }
}

// ========== 测试模块 ==========
#[cfg(test)]
mod tests;
