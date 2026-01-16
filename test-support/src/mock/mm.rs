//! 内存管理相关操作的 Mock 实现
//!
//! 注意：这里不直接依赖 `mm` crate（避免循环依赖）。
//! `mm` crate 在 `cfg(test)` 下为这些类型实现其 trait（例如 `ArchMmOps` / `MmConfig`）。

/// Mock 的内存管理架构操作
///
/// 默认实现采用“恒等映射”（vaddr == paddr），并提供最小可用语义以支持单元测试。
pub struct MockMmOps;

impl MockMmOps {
    pub const fn new() -> Self {
        Self
    }

    /// 将虚拟地址转换为物理地址（测试默认：恒等映射）
    ///
    /// # Safety
    /// 仅用于测试环境的可控输入。
    pub unsafe fn vaddr_to_paddr(&self, vaddr: usize) -> usize {
        vaddr
    }

    /// 将物理地址转换为虚拟地址（测试默认：恒等映射）
    pub fn paddr_to_vaddr(&self, paddr: usize) -> usize {
        paddr
    }

    /// sigreturn trampoline 的代码字节（测试默认：空）
    pub fn sigreturn_trampoline_bytes(&self) -> &'static [u8] {
        &[]
    }

    /// CPU 数量（测试默认：1）
    pub fn num_cpus(&self) -> usize {
        1
    }

    /// 发送 TLB flush IPI（测试默认：no-op）
    pub fn send_tlb_flush_ipi_all(&self) {}
}

/// 全局 Mock 实例
pub static MOCK_MM_OPS: MockMmOps = MockMmOps::new();

/// Mock 的内存管理配置
pub struct MockMmConfig;

impl MockMmConfig {
    pub const fn new() -> Self {
        Self
    }

    pub fn page_size(&self) -> usize {
        4096
    }

    pub fn memory_end(&self) -> usize {
        // 仅供测试：给一个较大的上界
        0x1_0000_0000
    }

    pub fn user_stack_size(&self) -> usize {
        1024 * 1024
    }

    pub fn user_stack_top(&self) -> usize {
        0x8000_0000
    }

    pub fn max_user_heap_size(&self) -> usize {
        128 * 1024 * 1024
    }

    pub fn user_sigreturn_trampoline(&self) -> usize {
        0
    }
}

/// 全局 Mock 实例
pub static MOCK_MM_CONFIG: MockMmConfig = MockMmConfig::new();

