//! Tmpfs Inode 实现
//!
//! TmpfsInode 直接管理物理帧，无需经过 BlockDevice 层

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::ops::fs_ops;
use mm::address::{ConvertablePaddr, PageNum, UsizeConvert};
use mm::frame_allocator::{FrameTracker, alloc_frame};
use sync::SpinLock;
use uapi::time::TimeSpec;
use vfs::{DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

/// Tmpfs Inode 实现
pub struct TmpfsInode {
    /// Inode 元数据
    metadata: SpinLock<InodeMetadata>,

    /// 文件数据页（稀疏存储）
    data: SpinLock<Vec<Option<Arc<FrameTracker>>>>,

    /// 父目录（弱引用，避免循环引用）
    parent: SpinLock<Weak<TmpfsInode>>,

    /// 子节点（仅对目录有效）
    children: SpinLock<BTreeMap<String, Arc<TmpfsInode>>>,

    /// Tmpfs 统计信息（共享引用）
    stats: Arc<SpinLock<TmpfsStats>>,

    /// 指向自身的弱引用
    self_ref: SpinLock<Weak<TmpfsInode>>,
}

/// Tmpfs 统计信息
#[derive(Debug, Clone)]
pub struct TmpfsStats {
    /// 已分配的总页数
    pub allocated_pages: usize,

    /// 最大允许的页数（0 表示无限制）
    pub max_pages: usize,

    /// 下一个 inode 编号
    pub next_inode_no: usize,
}

impl TmpfsInode {
    /// 创建新的 tmpfs inode
    pub fn new(
        inode_no: usize,
        inode_type: InodeType,
        mode: FileMode,
        parent: Weak<TmpfsInode>,
        stats: Arc<SpinLock<TmpfsStats>>,
    ) -> Arc<Self> {
        let now = fs_ops().timespec_now();

        let mode = mode & !FileMode::S_IFMT;

        let mode = match inode_type {
            InodeType::Directory => mode | FileMode::S_IFDIR,
            InodeType::File => mode | FileMode::S_IFREG,
            InodeType::Symlink => mode | FileMode::S_IFLNK,
            InodeType::CharDevice => mode | FileMode::S_IFCHR,
            InodeType::BlockDevice => mode | FileMode::S_IFBLK,
            InodeType::Fifo => mode | FileMode::S_IFIFO,
            InodeType::Socket => mode | FileMode::S_IFSOCK,
        };

        let metadata = InodeMetadata {
            inode_no,
            inode_type,
            mode,
            uid: 0,
            gid: 0,
            size: 0,
            atime: now,
            mtime: now,
            ctime: now,
            nlinks: if inode_type == InodeType::Directory { 2 } else { 1 },
            blocks: 0,
            rdev: 0,
        };

        Arc::new(Self {
            metadata: SpinLock::new(metadata),
            data: SpinLock::new(Vec::new()),
            parent: SpinLock::new(parent),
            children: SpinLock::new(BTreeMap::new()),
            stats,
            self_ref: SpinLock::new(Weak::new()),
        })
    }

    /// 创建根目录
    pub fn new_root(stats: Arc<SpinLock<TmpfsStats>>) -> Arc<Self> {
        let inode_no = {
            let mut stats_guard = stats.lock();
            let no = stats_guard.next_inode_no;
            stats_guard.next_inode_no += 1;
            no
        };

        let root = Self::new(
            inode_no,
            InodeType::Directory,
            FileMode::S_IRUSR
                | FileMode::S_IWUSR
                | FileMode::S_IXUSR
                | FileMode::S_IRGRP
                | FileMode::S_IXGRP
                | FileMode::S_IROTH
                | FileMode::S_IXOTH,
            Weak::new(),
            stats,
        );

        *root.self_ref.lock() = Arc::downgrade(&root);

        root
    }

    fn alloc_inode_no(&self) -> usize {
        let mut stats = self.stats.lock();
        let inode_no = stats.next_inode_no;
        stats.next_inode_no += 1;
        inode_no
    }

    fn dec_allocated_pages(&self, num: usize) {
        let mut stats = self.stats.lock();
        stats.allocated_pages = stats.allocated_pages.saturating_sub(num);
    }

    fn update_atime(&self) {
        let mut meta = self.metadata.lock();
        meta.atime = fs_ops().timespec_now();
    }

    fn update_mtime(&self) {
        let mut meta = self.metadata.lock();
        let now = fs_ops().timespec_now();
        meta.mtime = now;
        meta.ctime = now;
    }

    fn reserve_page(&self) -> Result<(), FsError> {
        let mut stats = self.stats.lock();
        if stats.max_pages != 0 && stats.allocated_pages >= stats.max_pages {
            return Err(FsError::NoSpace);
        }
        stats.allocated_pages += 1;
        Ok(())
    }

    fn cancel_page_reservation(&self) {
        self.dec_allocated_pages(1);
    }
}

impl Inode for TmpfsInode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(self.metadata.lock().clone())
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let meta = self.metadata.lock();

        if meta.inode_type != InodeType::File && meta.inode_type != InodeType::Symlink {
            return Err(FsError::IsDirectory);
        }

        if offset >= meta.size {
            return Ok(0);
        }

        let read_size = buf.len().min(meta.size - offset);
        drop(meta);

        let page_size = fs_ops().page_size();
        let mut bytes_read = 0;
        let data = self.data.lock();

        while bytes_read < read_size {
            let page_index = (offset + bytes_read) / page_size;
            let page_offset = (offset + bytes_read) % page_size;
            let read_len = (page_size - page_offset).min(read_size - bytes_read);

            if page_index >= data.len() || data[page_index].is_none() {
                buf[bytes_read..bytes_read + read_len].fill(0);
            } else {
                let frame = data[page_index].as_ref().unwrap();
                let kernel_vaddr = frame.ppn().start_addr().to_vaddr();

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        (kernel_vaddr.as_usize() + page_offset) as *const u8,
                        buf[bytes_read..].as_mut_ptr(),
                        read_len,
                    );
                }
            }

            bytes_read += read_len;
        }

        self.update_atime();
        Ok(bytes_read)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let meta = self.metadata.lock();

        if meta.inode_type != InodeType::File && meta.inode_type != InodeType::Symlink {
            return Err(FsError::IsDirectory);
        }

        drop(meta);

        let page_size = fs_ops().page_size();
        let mut data = self.data.lock();
        let mut bytes_written = 0;

        while bytes_written < buf.len() {
            let page_index = (offset + bytes_written) / page_size;
            let page_offset = (offset + bytes_written) % page_size;
            let write_len = (page_size - page_offset).min(buf.len() - bytes_written);

            if page_index >= data.len() {
                data.resize(page_index + 1, None);
            }

            if data[page_index].is_none() {
                if self.reserve_page().is_err() {
                    return Err(FsError::NoSpace);
                }

                match alloc_frame() {
                    Some(frame) => {
                        data[page_index] = Some(Arc::new(frame));
                    }
                    None => {
                        self.cancel_page_reservation();
                        return Err(FsError::NoSpace);
                    }
                }
            }

            let frame = data[page_index].as_ref().unwrap();
            let kernel_vaddr = frame.ppn().start_addr().to_vaddr();

            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf[bytes_written..].as_ptr(),
                    (kernel_vaddr.as_usize() + page_offset) as *mut u8,
                    write_len,
                );
            }

            bytes_written += write_len;
        }

        drop(data);

        let mut meta = self.metadata.lock();
        meta.size = meta.size.max(offset + bytes_written);
        meta.blocks = (meta.size + 511) / 512;
        drop(meta);

        self.update_mtime();
        Ok(bytes_written)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let children = self.children.lock();

        if name == "." {
            let self_weak = self.self_ref.lock();
            if let Some(self_arc) = self_weak.upgrade() {
                return Ok(self_arc as Arc<dyn Inode>);
            }
            return Err(FsError::IoError);
        } else if name == ".." {
            let parent = self.parent.lock();
            if let Some(parent_arc) = parent.upgrade() {
                return Ok(parent_arc as Arc<dyn Inode>);
            }
            let self_weak = self.self_ref.lock();
            if let Some(self_arc) = self_weak.upgrade() {
                return Ok(self_arc as Arc<dyn Inode>);
            }
            return Err(FsError::IoError);
        }

        children
            .get(name)
            .cloned()
            .map(|inode| inode as Arc<dyn Inode>)
            .ok_or(FsError::NotFound)
    }

    fn create(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        let inode_no = self.alloc_inode_no();
        let parent_weak = self.self_ref.lock().clone();

        let new_inode = TmpfsInode::new(
            inode_no,
            InodeType::File,
            mode,
            parent_weak,
            self.stats.clone(),
        );

        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        children.insert(String::from(name), new_inode.clone());
        drop(children);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn mkdir(&self, name: &str, mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        let inode_no = self.alloc_inode_no();
        let parent_weak = self.self_ref.lock().clone();

        let new_inode = TmpfsInode::new(
            inode_no,
            InodeType::Directory,
            mode,
            parent_weak,
            self.stats.clone(),
        );

        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        children.insert(String::from(name), new_inode.clone());
        drop(children);

        let mut meta = self.metadata.lock();
        meta.nlinks += 1;
        drop(meta);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        let child = children.get(name).ok_or(FsError::NotFound)?;

        let child_meta = child.metadata.lock();
        if child_meta.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }
        drop(child_meta);

        let child_data = child.data.lock();
        let allocated = child_data.iter().filter(|f: &&Option<_>| f.is_some()).count();
        drop(child_data);

        children.remove(name);
        self.dec_allocated_pages(allocated);
        self.update_mtime();

        Ok(())
    }

    fn rmdir(&self, name: &str) -> Result<(), FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();

        let child = children.get(name).ok_or(FsError::NotFound)?;

        let child_meta = child.metadata.lock();
        if child_meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(child_meta);

        let child_children = child.children.lock();
        if !child_children.is_empty() {
            return Err(FsError::DirectoryNotEmpty);
        }
        drop(child_children);

        children.remove(name);

        let mut meta = self.metadata.lock();
        meta.nlinks = meta.nlinks.saturating_sub(1);
        drop(meta);

        self.update_mtime();

        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        let inode_no = meta.inode_no;
        drop(meta);

        let children = self.children.lock();
        let mut entries = Vec::new();

        entries.push(DirEntry {
            name: String::from("."),
            inode_no,
            inode_type: InodeType::Directory,
        });

        let parent = self.parent.lock();
        let parent_inode_no = if let Some(parent_arc) = parent.upgrade() {
            parent_arc.metadata.lock().inode_no
        } else {
            inode_no
        };
        drop(parent);

        entries.push(DirEntry {
            name: String::from(".."),
            inode_no: parent_inode_no,
            inode_type: InodeType::Directory,
        });

        for (name, child) in children.iter() {
            let child_meta = child.metadata.lock();
            entries.push(DirEntry {
                name: String::clone(name),
                inode_no: child_meta.inode_no,
                inode_type: child_meta.inode_type,
            });
        }

        Ok(entries)
    }

    fn truncate(&self, new_size: usize) -> Result<(), FsError> {
        let mut meta = self.metadata.lock();

        if meta.inode_type != InodeType::File {
            return Err(FsError::IsDirectory);
        }

        let old_size = meta.size;
        let page_size = fs_ops().page_size();

        if new_size < old_size {
            let new_page_count = (new_size + page_size - 1) / page_size;
            let old_page_count = (old_size + page_size - 1) / page_size;

            let mut data = self.data.lock();

            let pages_to_free = data[new_page_count..old_page_count.min(data.len())]
                .iter()
                .filter(|f: &&Option<_>| f.is_some())
                .count();

            data.truncate(new_page_count);
            drop(data);

            self.dec_allocated_pages(pages_to_free);
        }

        meta.size = new_size;
        meta.blocks = (new_size + 511) / 512;
        drop(meta);

        self.update_mtime();
        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }
        drop(children);

        let inode_no = self.alloc_inode_no();

        let symlink_inode = TmpfsInode::new(
            inode_no,
            InodeType::Symlink,
            FileMode::from_bits_truncate(0o777),
            Arc::downgrade(&self.self_ref.lock().upgrade().unwrap()),
            self.stats.clone(),
        );

        let target_bytes = target.as_bytes();
        if let Err(e) = symlink_inode.write_at(0, target_bytes) {
            return Err(e);
        }

        self.children
            .lock()
            .insert(name.to_string(), symlink_inode.clone());

        self.metadata.lock().mtime = fs_ops().timespec_now();

        Ok(symlink_inode as Arc<dyn Inode>)
    }

    fn link(&self, _name: &str, _target: &Arc<dyn Inode>) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn rename(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
    ) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError> {
        let mut metadata = self.metadata.lock();
        if let Some(atime) = atime {
            metadata.atime = atime;
        }
        if let Some(mtime) = mtime {
            metadata.mtime = mtime;
        }
        Ok(())
    }

    fn readlink(&self) -> Result<String, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Symlink {
            return Err(FsError::InvalidArgument);
        }
        let size = meta.size;
        drop(meta);

        if size == 0 {
            return Ok(String::new());
        }

        let mut buf = alloc::vec![0u8; size];
        let bytes_read = self.read_at(0, &mut buf)?;

        String::from_utf8(buf[..bytes_read].to_vec()).map_err(|_| FsError::InvalidArgument)
    }

    fn mknod(&self, name: &str, mode: FileMode, dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        let meta = self.metadata.lock();
        if meta.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }
        drop(meta);

        let mut children = self.children.lock();
        if children.contains_key(name) {
            return Err(FsError::AlreadyExists);
        }

        let inode_type = if mode.contains(FileMode::S_IFCHR) {
            InodeType::CharDevice
        } else if mode.contains(FileMode::S_IFBLK) {
            InodeType::BlockDevice
        } else if mode.contains(FileMode::S_IFIFO) {
            InodeType::Fifo
        } else {
            return Err(FsError::InvalidArgument);
        };

        let inode_no = self.alloc_inode_no();
        let parent_weak = self.self_ref.lock().clone();

        let new_inode =
            TmpfsInode::new(inode_no, inode_type, mode, parent_weak, self.stats.clone());

        new_inode.metadata.lock().rdev = dev;
        *new_inode.self_ref.lock() = Arc::downgrade(&new_inode);

        children.insert(String::from(name), new_inode.clone());
        drop(children);

        self.update_mtime();

        Ok(new_inode as Arc<dyn Inode>)
    }

    fn chmod(&self, _mode: FileMode) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn chown(&self, _uid: u32, _gid: u32) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
