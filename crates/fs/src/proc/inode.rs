//! ProcFS Inode 实现

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use sync::SpinLock;
use uapi::time::TimeSpec;
use vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

use crate::ops::fs_ops;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcInodeKind {
    Generic,
    Root,
    PidDir(u32),
}

const PROC_PID_INO_BASE: usize = 1_000_000_000;
const PROC_PID_INO_STRIDE: usize = 32;

fn proc_pid_dir_inode_no(pid: u32) -> usize {
    PROC_PID_INO_BASE + (pid as usize).saturating_mul(PROC_PID_INO_STRIDE)
}

fn proc_pid_child_inode_no(pid: u32, offset: usize) -> usize {
    proc_pid_dir_inode_no(pid).saturating_add(offset)
}

/// 动态内容生成器 trait
pub trait ContentGenerator: Send + Sync {
    /// 生成文件内容（每次调用时重新生成）
    fn generate(&self) -> Result<Vec<u8>, FsError>;
}

/// ProcFS 中的 inode 节点。
///
/// 通过 [`ProcInodeContent`] 表示文件/目录/符号链接等不同内容形态。
pub struct ProcInode {
    kind: ProcInodeKind,
    metadata: SpinLock<InodeMetadata>,
    content: ProcInodeContent,
}

/// ProcFS inode 的内容类型。
pub enum ProcInodeContent {
    /// 静态文件（内容固定）
    Static(Vec<u8>),
    /// 动态文件（每次读取时生成）
    Dynamic(Arc<dyn ContentGenerator>),
    /// 目录（包含子节点）
    Directory(SpinLock<BTreeMap<String, Arc<ProcInode>>>),
    /// 符号链接
    Symlink(String),
    /// 动态符号链接（每次读取时生成目标）
    DynamicSymlink(Arc<dyn Fn() -> String + Send + Sync>),
}

/// 全局 Inode 编号分配器
static NEXT_INODE_NO: AtomicUsize = AtomicUsize::new(1);

impl ProcInode {
    /// 创建 `/proc` 根目录 inode。
    pub fn new_proc_root_directory(mode: FileMode) -> Arc<Self> {
        Self::new_directory_with_inode_no(mode, None, ProcInodeKind::Root)
    }

    /// 创建静态文件 inode
    pub fn new_static_file(_name: &str, content: Vec<u8>, mode: FileMode) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = fs_ops().timespec_now();

        Arc::new(Self {
            kind: ProcInodeKind::Generic,
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::File,
                mode,
                uid: 0,
                gid: 0,
                size: 0,
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Static(content),
        })
    }

    /// 创建动态文件 inode
    pub fn new_dynamic_file(
        _name: &str,
        generator: Arc<dyn ContentGenerator>,
        mode: FileMode,
    ) -> Arc<Self> {
        Self::new_dynamic_file_with_inode_no(generator, mode, None)
    }

    fn new_dynamic_file_with_inode_no(
        generator: Arc<dyn ContentGenerator>,
        mode: FileMode,
        inode_no: Option<usize>,
    ) -> Arc<Self> {
        let inode_no = inode_no.unwrap_or_else(|| NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed));
        let now = fs_ops().timespec_now();

        Arc::new(Self {
            kind: ProcInodeKind::Generic,
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::File,
                mode,
                uid: 0,
                gid: 0,
                size: 0,
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Dynamic(generator),
        })
    }

    /// 创建目录 inode
    pub fn new_directory(mode: FileMode) -> Arc<Self> {
        Self::new_directory_with_inode_no(mode, None, ProcInodeKind::Generic)
    }

    fn new_directory_with_inode_no(
        mode: FileMode,
        inode_no: Option<usize>,
        kind: ProcInodeKind,
    ) -> Arc<Self> {
        let inode_no = inode_no.unwrap_or_else(|| NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed));
        let now = fs_ops().timespec_now();

        Arc::new(Self {
            kind,
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Directory,
                mode,
                uid: 0,
                gid: 0,
                size: 0,
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 2,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Directory(SpinLock::new(BTreeMap::new())),
        })
    }

    /// 创建符号链接 inode
    pub fn new_symlink(_name: &str, target: String) -> Arc<Self> {
        let inode_no = NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed);
        let now = fs_ops().timespec_now();

        Arc::new(Self {
            kind: ProcInodeKind::Generic,
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Symlink,
                mode: FileMode::from_bits_truncate(0o777),
                uid: 0,
                gid: 0,
                size: target.len(),
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::Symlink(target),
        })
    }

    /// 创建动态符号链接 inode
    pub fn new_dynamic_symlink<F>(_name: &str, generator: F) -> Arc<Self>
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self::new_dynamic_symlink_with_inode_no(generator, None)
    }

    fn new_dynamic_symlink_with_inode_no<F>(generator: F, inode_no: Option<usize>) -> Arc<Self>
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        let inode_no = inode_no.unwrap_or_else(|| NEXT_INODE_NO.fetch_add(1, Ordering::Relaxed));
        let now = fs_ops().timespec_now();

        Arc::new(Self {
            kind: ProcInodeKind::Generic,
            metadata: SpinLock::new(InodeMetadata {
                inode_no,
                inode_type: InodeType::Symlink,
                mode: FileMode::from_bits_truncate(0o777),
                uid: 0,
                gid: 0,
                size: 0,
                atime: now,
                mtime: now,
                ctime: now,
                nlinks: 1,
                blocks: 0,
                rdev: 0,
            }),
            content: ProcInodeContent::DynamicSymlink(Arc::new(generator)),
        })
    }

    /// 向目录添加子节点
    pub fn add_child(&self, name: &str, child: Arc<ProcInode>) -> Result<(), FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                children.lock().insert(name.to_string(), child);
                Ok(())
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    /// 为指定 PID 创建进程目录
    fn create_process_dir(&self, pid: u32) -> Option<Arc<ProcInode>> {
        use crate::proc::generators::{
            CmdlineGenerator, MapsGenerator, StatGenerator, StatusGenerator,
        };

        let task = fs_ops().get_task(pid)?;

        let proc_dir = Self::new_directory_with_inode_no(
            FileMode::from_bits_truncate(0o555 | FileMode::S_IFDIR.bits()),
            Some(proc_pid_dir_inode_no(pid)),
            ProcInodeKind::PidDir(pid),
        );

        // 创建 status 文件
        let status = Self::new_dynamic_file_with_inode_no(
            Arc::new(StatusGenerator::new(pid)),
            FileMode::from_bits_truncate(0o444),
            Some(proc_pid_child_inode_no(pid, 1)),
        );
        let _ = proc_dir.add_child("status", status);

        // 创建 stat 文件
        let stat = Self::new_dynamic_file_with_inode_no(
            Arc::new(StatGenerator::new(pid)),
            FileMode::from_bits_truncate(0o444),
            Some(proc_pid_child_inode_no(pid, 2)),
        );
        let _ = proc_dir.add_child("stat", stat);

        // 创建 cmdline 文件
        let cmdline = Self::new_dynamic_file_with_inode_no(
            Arc::new(CmdlineGenerator::new(pid)),
            FileMode::from_bits_truncate(0o444),
            Some(proc_pid_child_inode_no(pid, 3)),
        );
        let _ = proc_dir.add_child("cmdline", cmdline);

        // 创建 exe 符号链接
        let exe = Self::new_dynamic_symlink_with_inode_no(
            move || {
                fs_ops()
                    .get_task(pid)
                    .and_then(|t| t.exe_path())
                    .unwrap_or_else(|| "/".to_string())
            },
            Some(proc_pid_child_inode_no(pid, 4)),
        );
        let _ = proc_dir.add_child("exe", exe);

        // 创建 maps 文件
        let maps = Self::new_dynamic_file_with_inode_no(
            Arc::new(MapsGenerator::new(pid)),
            FileMode::from_bits_truncate(0o444),
            Some(proc_pid_child_inode_no(pid, 5)),
        );
        let _ = proc_dir.add_child("maps", maps);

        // 验证任务仍然存在
        let _ = task;

        Some(proc_dir)
    }
}

impl Inode for ProcInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let mut meta = self.metadata.lock().clone();
        if meta.inode_type == InodeType::Symlink {
            meta.size = match &self.content {
                ProcInodeContent::Symlink(target) => target.len(),
                ProcInodeContent::DynamicSymlink(generator) => generator().len(),
                _ => meta.size,
            };
        }
        Ok(meta)
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        match &self.content {
            ProcInodeContent::Static(data) => {
                if offset >= data.len() {
                    return Ok(0);
                }
                let to_read = (data.len() - offset).min(buf.len());
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            ProcInodeContent::Dynamic(generator) => {
                let data = generator.generate()?;
                if offset >= data.len() {
                    return Ok(0);
                }
                let to_read = (data.len() - offset).min(buf.len());
                buf[..to_read].copy_from_slice(&data[offset..offset + to_read]);
                Ok(to_read)
            }
            _ => Err(FsError::IsDirectory),
        }
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                if let Some(child) = children.lock().get(name).cloned() {
                    return Ok(child as Arc<dyn Inode>);
                }

                if self.kind == ProcInodeKind::Root {
                    if let Ok(pid) = name.parse::<u32>() {
                        if let Some(proc_dir) = self.create_process_dir(pid) {
                            return Ok(proc_dir as Arc<dyn Inode>);
                        }
                    }
                }

                Err(FsError::NotFound)
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    fn create(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn mkdir(&self, _name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn symlink(&self, _name: &str, _target: &str) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn unlink(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rmdir(&self, _name: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        match &self.content {
            ProcInodeContent::Directory(children) => {
                let metadata = self.metadata.lock();
                let mut entries = Vec::new();

                entries.push(DirEntry {
                    name: ".".to_string(),
                    inode_no: metadata.inode_no,
                    inode_type: InodeType::Directory,
                });
                entries.push(DirEntry {
                    name: "..".to_string(),
                    inode_no: metadata.inode_no,
                    inode_type: InodeType::Directory,
                });

                let child_names: Vec<String> = {
                    let guard = children.lock();
                    for (name, child) in guard.iter() {
                        let child_meta = child.metadata.lock();
                        entries.push(DirEntry {
                            name: String::clone(name),
                            inode_no: child_meta.inode_no,
                            inode_type: child_meta.inode_type,
                        });
                    }
                    guard.keys().cloned().collect()
                };

                if self.kind == ProcInodeKind::Root {
                    let pids = fs_ops().list_process_pids();
                    for pid in pids {
                        let name = pid.to_string();
                        if child_names.binary_search(&name).is_ok() {
                            continue;
                        }
                        entries.push(DirEntry {
                            name,
                            inode_no: proc_pid_dir_inode_no(pid),
                            inode_type: InodeType::Directory,
                        });
                    }
                }

                Ok(entries)
            }
            _ => Err(FsError::NotDirectory),
        }
    }

    fn cacheable(&self) -> bool {
        !matches!(self.kind, ProcInodeKind::PidDir(_))
    }

    fn truncate(&self, _size: usize) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn set_times(&self, _atime: Option<TimeSpec>, _mtime: Option<TimeSpec>) -> Result<(), FsError> {
        Err(FsError::PermissionDenied)
    }

    fn readlink(&self) -> Result<String, FsError> {
        match &self.content {
            ProcInodeContent::Symlink(target) => Ok(target.clone()),
            ProcInodeContent::DynamicSymlink(generator) => Ok(generator()),
            _ => Err(FsError::InvalidArgument),
        }
    }

    fn mknod(&self, _name: &str, _mode: FileMode, _dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
