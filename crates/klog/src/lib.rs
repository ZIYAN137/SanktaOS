//! 内核日志子系统
//!
//! 该模块提供了一个类似 **Linux 内核风格的日志系统**，并在裸机环境中实现了**无锁环形缓冲区**。
//!
//! # 组件
//!
//! - [`buffer`] - 用于日志存储的无锁环形缓冲区
//! - [`config`] - 配置常量（缓冲区大小、消息长度限制）
//! - [`log_core`] - 核心日志实现 (LogCore)
//! - [`entry`] - 日志条目结构和序列化
//! - [`level`] - 日志级别定义（从 Emergency 到 Debug）
//! - [`macros`] - 面向用户的日志宏 (`pr_info!`, `pr_err!`, 等)
//!
//! # 设计概览
//!
//! ## 双输出策略
//!
//! 日志系统采用两层方法：
//!
//! 1. **即时控制台输出**：达到控制台级别阈值（默认：Warning 及以上）的日志会**直接打印到控制台**，以实现紧急可见性。
//! 2. **环形缓冲区存储**：所有达到全局级别阈值（默认：Info 及以上）的日志都会被写入**无锁环形缓冲区**，用于异步消费或事后分析。
//!
//! ## 性能特点
//!
//! - **无锁并发**：使用原子操作（fetch_add, CAS）而非互斥锁，支持多生产者日志记录而**不会阻塞**。
//! - **早期过滤**：日志级别检查在宏展开时发生，避免对禁用级别的日志进行格式化字符串评估。
//! - **固定大小分配**：**没有动态内存分配**；所有结构体使用编译时已知的大小，适用于裸机环境。
//! - **缓存优化**：读写器数据结构经过缓存行填充（64 字节），以防止多核系统上的**伪共享**。
//! - **尽可能零拷贝**：在可行的情况下，日志条目是**就地构造**的，以最大限度地减少内存操作。
//!
//! # 架构解耦
//!
//! 日志系统通过 trait 抽象与架构特定组件解耦：
//!
//! - **LogContextProvider**：提供 CPU ID、任务 ID、时间戳
//! - **LogOutput**：提供控制台输出能力
//!
//! 使用方需要在启动时注册这些 trait 的实现。

#![no_std]
#![allow(unused)]

extern crate alloc;

mod buffer;
mod config;
mod entry;
mod level;
mod log_core;
pub mod macros;

pub use config::{
    DEFAULT_CONSOLE_LEVEL, DEFAULT_LOG_LEVEL, GLOBAL_LOG_BUFFER_SIZE, MAX_LOG_MESSAGE_LENGTH,
};
pub use entry::LogEntry;
pub use level::LogLevel;
pub use log_core::format_log_entry;

use core::sync::atomic::{AtomicPtr, Ordering};

// ========== Trait 定义 ==========

/// 日志上下文提供者 trait
///
/// 实现此 trait 以提供日志所需的上下文信息（CPU ID、任务 ID、时间戳）。
/// 使用方需要在启动时通过 `register_context_provider` 注册实现。
pub trait LogContextProvider: Send + Sync {
    /// 获取当前 CPU ID
    fn cpu_id(&self) -> usize;
    /// 获取当前任务 ID（如果没有任务则返回 0）
    fn task_id(&self) -> u32;
    /// 获取当前时间戳
    fn timestamp(&self) -> usize;
}

/// 日志输出 trait
///
/// 实现此 trait 以提供日志的控制台输出能力。
/// 使用方需要在启动时通过 `register_log_output` 注册实现。
pub trait LogOutput: Send + Sync {
    /// 输出字符串到控制台
    fn write_str(&self, s: &str);
}

// ========== 全局注册机制 ==========

/// 存储 LogContextProvider trait object 的胖指针
struct ContextProviderPtr {
    data: AtomicPtr<()>,
    vtable: AtomicPtr<()>,
}

impl ContextProviderPtr {
    const fn new() -> Self {
        Self {
            data: AtomicPtr::new(core::ptr::null_mut()),
            vtable: AtomicPtr::new(core::ptr::null_mut()),
        }
    }
}

/// 存储 LogOutput trait object 的胖指针
struct LogOutputPtr {
    data: AtomicPtr<()>,
    vtable: AtomicPtr<()>,
}

impl LogOutputPtr {
    const fn new() -> Self {
        Self {
            data: AtomicPtr::new(core::ptr::null_mut()),
            vtable: AtomicPtr::new(core::ptr::null_mut()),
        }
    }
}

static CONTEXT_PROVIDER: ContextProviderPtr = ContextProviderPtr::new();
static LOG_OUTPUT: LogOutputPtr = LogOutputPtr::new();

/// 注册日志上下文提供者
///
/// # Safety
///
/// - 必须在任何日志调用之前调用
/// - provider 必须具有 'static 生命周期
/// - 只能调用一次
pub unsafe fn register_context_provider(provider: &'static dyn LogContextProvider) {
    let ptr: *const dyn LogContextProvider = provider;
    let (data, vtable) = unsafe { core::mem::transmute::<_, (*mut (), *mut ())>(ptr) };
    CONTEXT_PROVIDER.data.store(data, Ordering::Release);
    CONTEXT_PROVIDER.vtable.store(vtable, Ordering::Release);
}

/// 注册日志输出
///
/// # Safety
///
/// - 必须在任何日志调用之前调用
/// - output 必须具有 'static 生命周期
/// - 只能调用一次
pub unsafe fn register_log_output(output: &'static dyn LogOutput) {
    let ptr: *const dyn LogOutput = output;
    let (data, vtable) = unsafe { core::mem::transmute::<_, (*mut (), *mut ())>(ptr) };
    LOG_OUTPUT.data.store(data, Ordering::Release);
    LOG_OUTPUT.vtable.store(vtable, Ordering::Release);
}

/// 获取已注册的上下文提供者
pub(crate) fn get_context_provider() -> Option<&'static dyn LogContextProvider> {
    let data = CONTEXT_PROVIDER.data.load(Ordering::Acquire);
    let vtable = CONTEXT_PROVIDER.vtable.load(Ordering::Acquire);
    if data.is_null() || vtable.is_null() {
        return None;
    }
    // Safety: 指针由 register_context_provider 设置，保证有效
    Some(unsafe {
        core::mem::transmute::<(*mut (), *mut ()), &'static dyn LogContextProvider>((data, vtable))
    })
}

/// 获取已注册的日志输出
pub(crate) fn get_log_output() -> Option<&'static dyn LogOutput> {
    let data = LOG_OUTPUT.data.load(Ordering::Acquire);
    let vtable = LOG_OUTPUT.vtable.load(Ordering::Acquire);
    if data.is_null() || vtable.is_null() {
        return None;
    }
    // Safety: 指针由 register_log_output 设置，保证有效
    Some(unsafe {
        core::mem::transmute::<(*mut (), *mut ()), &'static dyn LogOutput>((data, vtable))
    })
}

// ========== 全局单例 ==========

/// 全局日志系统实例
///
/// 使用 const fn 在编译时初始化，零运行时开销。
/// 所有日志宏和公共 API 都委托给此实例。
static GLOBAL_LOG: log_core::LogCore = log_core::LogCore::default();

// ========== 公共 API (精简封装) ==========

/// 核心日志实现（由宏调用）
#[doc(hidden)]
pub fn log_impl(level: LogLevel, args: core::fmt::Arguments) {
    GLOBAL_LOG._log(level, args);
}

/// 检查日志级别是否启用（由宏调用）
#[doc(hidden)]
pub fn is_level_enabled(level: LogLevel) -> bool {
    level as u8 <= GLOBAL_LOG._get_global_level() as u8
}

/// 从缓冲区读取下一个日志条目
pub fn read_log() -> Option<LogEntry> {
    GLOBAL_LOG._read_log()
}

/// 非破坏性读取：按索引 peek 日志条目，不移动读指针
pub fn peek_log(index: usize) -> Option<LogEntry> {
    GLOBAL_LOG._peek_log(index)
}

/// 获取当前可读取的起始索引
pub fn log_reader_index() -> usize {
    GLOBAL_LOG._log_reader_index()
}

/// 获取当前写入位置
pub fn log_writer_index() -> usize {
    GLOBAL_LOG._log_writer_index()
}

/// 返回未读日志条目的数量
pub fn log_len() -> usize {
    GLOBAL_LOG._log_len()
}

/// 返回未读日志的总字节数（格式化后）
pub fn log_unread_bytes() -> usize {
    GLOBAL_LOG._log_unread_bytes()
}

/// 返回已丢弃日志的计数
pub fn log_dropped_count() -> usize {
    GLOBAL_LOG._log_dropped_count()
}

/// 设置全局日志级别阈值
pub fn set_global_level(level: LogLevel) {
    GLOBAL_LOG._set_global_level(level);
}

/// 获取当前全局日志级别
pub fn get_global_level() -> LogLevel {
    GLOBAL_LOG._get_global_level()
}

/// 设置控制台输出级别阈值
pub fn set_console_level(level: LogLevel) {
    GLOBAL_LOG._set_console_level(level);
}

/// 获取当前控制台输出级别
pub fn get_console_level() -> LogLevel {
    GLOBAL_LOG._get_console_level()
}
