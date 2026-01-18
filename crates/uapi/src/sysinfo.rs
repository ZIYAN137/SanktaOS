use core::ffi::{c_long, c_uint, c_ulong};
use core::mem::size_of;

/// 系统信息结构体
/// 对应 Linux 的 `struct sysinfo`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SysInfo {
    /// 系统启动后经过的时间，单位为秒
    pub uptime: c_long,
    /// 1 分钟、5 分钟和 15 分钟的平均负载
    pub loads: [c_ulong; 3],
    /// 总内存大小，单位为字节
    pub totalram: c_ulong,
    /// 可用内存大小，单位为字节
    pub freeram: c_ulong,
    /// 缓存大小，单位为字节
    pub sharedram: c_ulong,
    /// 用作文件缓存的内存大小，单位为字节
    pub bufferram: c_ulong,
    /// 总交换空间大小，单位为字节
    pub totalswap: c_ulong,
    /// 可用交换空间大小，单位为字节
    pub freeswap: c_ulong,
    /// 当前进程数
    pub procs: u16,
    /// 显式 padding（与 Linux UAPI 一致，历史上用于 m68k）
    pub pad: u16,
    /// 高端内存总大小，单位为字节
    pub totalhigh: c_ulong,
    /// 高端可用内存大小，单位为字节
    pub freehigh: c_ulong,
    /// 内存单位大小，单位为字节
    pub mem_unit: c_uint,
    /// Padding: libc5 uses this. See `include/uapi/linux/sysinfo.h`.
    pub _f: [u8; Self::F_LEN],
}

impl SysInfo {
    // `include/uapi/linux/sysinfo.h`:
    //   char _f[20-2*sizeof(__kernel_ulong_t)-sizeof(__u32)];
    // For 64-bit this is 0, but keep it generic for completeness.
    const F_LEN: usize = 20 - 2 * size_of::<c_ulong>() - size_of::<c_uint>();

    /// 创建一个新的 SysInfo 实例，所有字段（包括 padding）初始化为零
    pub fn new() -> Self {
        // SAFETY: all-zero is a valid bit-pattern for SysInfo, and it ensures
        // we don't leak uninitialized padding to userspace when copying out.
        unsafe { core::mem::zeroed() }
    }
}
