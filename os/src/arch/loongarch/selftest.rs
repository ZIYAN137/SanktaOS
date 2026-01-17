//! LoongArch64 self-tests
//!
//! 这些测试只在 `cfg(test)` 下编译，并由内核的测试运行器执行。

#[cfg(test)]
mod tests {
    use loongArch64::register::{CpuMode, crmd};
    #[test_case]
    fn test_loongarch_smoke() {
        // Basic CSR sanity: should be in PLV0 at boot.
        let plv = crmd::read().plv();
        assert!(matches!(plv, CpuMode::Ring0));

        // Trap trampoline symbol should be linked in.
        let sigret = crate::arch::trap::sigreturn_trampoline_address();
        assert!(sigret != 0);
        assert!(sigret & 0x3 == 0);

        // Syscall numbers: asm-generic aligned sanity checks.
        assert!(crate::arch::syscall::SYS_READ == 63);
        assert!(crate::arch::syscall::SYS_WRITE == 64);
        assert!(crate::arch::syscall::SYS_EXIT == 93);
    }
}
