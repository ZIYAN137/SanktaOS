//! VFS 错误类型
//!
//! 定义了与 POSIX 兼容的文件系统错误码，可通过 [`FsError::to_errno()`] 转换为系统调用错误码。

/// VFS 错误类型
///
/// 各错误码对应标准 POSIX errno 值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    // 文件/目录相关
    /// 文件不存在 (-ENOENT)
    NotFound,
    /// 文件已存在 (-EEXIST)
    AlreadyExists,
    /// 不是目录 (-ENOTDIR)
    NotDirectory,
    /// 是目录 (-EISDIR)
    IsDirectory,
    /// 目录非空 (-ENOTEMPTY)
    DirectoryNotEmpty,

    // 权限相关
    /// 权限被拒绝 (-EACCES)
    PermissionDenied,

    // 文件描述符相关
    /// 无效的文件描述符 (-EBADF)
    BadFileDescriptor,
    /// 打开的文件过多 (-EMFILE)
    TooManyOpenFiles,

    // 参数相关
    /// 无效参数 (-EINVAL)
    InvalidArgument,
    /// 文件名过长 (-ENAMETOOLONG)
    NameTooLong,

    // 文件系统相关
    /// 只读文件系统 (-EROFS)
    ReadOnlyFs,
    /// 设备空间不足 (-ENOSPC)
    NoSpace,
    /// I/O 错误 (-EIO)
    IoError,
    /// 设备不存在 (-ENODEV)
    NoDevice,

    // 管道相关
    /// 管道破裂 (-EPIPE)
    BrokenPipe,
    /// 非阻塞操作将阻塞 (-EAGAIN)
    WouldBlock,

    // 网络相关
    /// 套接字未连接 (-ENOTCONN)
    NotConnected,

    // 其他
    /// 操作不支持 (-ENOTSUP)
    NotSupported,
    /// 硬链接过多 (-EMLINK)
    TooManyLinks,
    /// 符号链接层级过多 (-ELOOP)
    TooManySymlinks,
}

impl FsError {
    /// 转换为系统调用错误码（负数）
    pub fn to_errno(&self) -> isize {
        match self {
            FsError::NotFound => -2,
            FsError::IoError => -5,
            FsError::BadFileDescriptor => -9,
            FsError::WouldBlock => -11,
            FsError::PermissionDenied => -13,
            FsError::AlreadyExists => -17,
            FsError::NoDevice => -19,
            FsError::NotDirectory => -20,
            FsError::IsDirectory => -21,
            FsError::InvalidArgument => -22,
            FsError::TooManyOpenFiles => -24,
            FsError::NoSpace => -28,
            FsError::ReadOnlyFs => -30,
            FsError::TooManyLinks => -31,
            FsError::TooManySymlinks => -40,
            FsError::BrokenPipe => -32,
            FsError::NameTooLong => -36,
            FsError::DirectoryNotEmpty => -39,
            FsError::NotSupported => -95,
            FsError::NotConnected => -107,
        }
    }
}
