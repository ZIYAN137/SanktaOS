//! Ext4 Inode 包装
//!
//! 将 ext4_rs 的 inode 操作包装为 VFS Inode trait

use crate::ops::fs_ops;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use ext4_rs::InodeFileType;
use sync::SpinLock;
use uapi::time::TimeSpec;

use vfs::{Dentry, DirEntry, FileMode, FsError, Inode, InodeMetadata, InodeType};

/// Ext4 Inode 包装
pub struct Ext4Inode {
    /// ext4_rs 文件系统对象
    fs: Arc<SpinLock<ext4_rs::Ext4>>,

    /// Inode 号
    ino: u32,

    /// 关联的 Dentry（弱引用，避免循环引用）
    dentry: SpinLock<Weak<Dentry>>,
}

impl Ext4Inode {
    /// 创建新的 Ext4Inode
    pub fn new(fs: Arc<SpinLock<ext4_rs::Ext4>>, ino: u32) -> Self {
        Self {
            fs,
            ino,
            dentry: SpinLock::new(Weak::new()),
        }
    }

    /// 辅助方法：获取完整路径（从 Dentry 动态获取）
    fn get_full_path(&self) -> Result<String, FsError> {
        let dentry = self.dentry.lock().upgrade().ok_or(FsError::IoError)?;
        Ok(dentry.full_path())
    }

    /// 辅助方法：将 ext4_rs 的 InodeFileType 转换为 VFS InodeType
    fn convert_inode_type(ft: ext4_rs::InodeFileType) -> InodeType {
        use ext4_rs::InodeFileType;
        match ft {
            InodeFileType::S_IFREG => InodeType::File,
            InodeFileType::S_IFDIR => InodeType::Directory,
            InodeFileType::S_IFLNK => InodeType::Symlink,
            InodeFileType::S_IFCHR => InodeType::CharDevice,
            InodeFileType::S_IFBLK => InodeType::BlockDevice,
            InodeFileType::S_IFIFO => InodeType::Fifo,
            InodeFileType::S_IFSOCK => InodeType::Socket,
            _ => InodeType::File,
        }
    }

    /// 辅助方法：将ext4_rs的DirEntryType转换为VFS InodeType
    fn convert_dir_entry_type(dentry_type: u8) -> InodeType {
        match dentry_type {
            1 => InodeType::File,
            2 => InodeType::Directory,
            3 => InodeType::CharDevice,
            4 => InodeType::BlockDevice,
            5 => InodeType::Fifo,
            6 => InodeType::Socket,
            7 => InodeType::Symlink,
            _ => InodeType::File,
        }
    }
}

impl Inode for Ext4Inode {
    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        let fs = self.fs.lock();

        let inode_ref = fs.get_inode_ref(self.ino);
        let inode = &inode_ref.inode;

        let size = (inode.size as u64) | ((inode.size_hi as u64) << 32);

        let mode = inode.mode;
        let file_type = (mode & 0xF000) >> 12;
        let inode_type = match file_type {
            0x8 => InodeType::File,
            0x4 => InodeType::Directory,
            0xA => InodeType::Symlink,
            0x2 => InodeType::CharDevice,
            0x6 => InodeType::BlockDevice,
            0x1 => InodeType::Fifo,
            0xC => InodeType::Socket,
            _ => InodeType::File,
        };

        let atime_nsec = (inode.i_atime_extra >> 2) as i64;
        let mtime_nsec = (inode.i_mtime_extra >> 2) as i64;
        let ctime_nsec = (inode.i_ctime_extra >> 2) as i64;

        Ok(InodeMetadata {
            inode_no: self.ino as usize,
            size: size as usize,
            blocks: inode.blocks as usize,
            atime: TimeSpec {
                tv_sec: inode.atime as i64,
                tv_nsec: atime_nsec,
            },
            mtime: TimeSpec {
                tv_sec: inode.mtime as i64,
                tv_nsec: mtime_nsec,
            },
            ctime: TimeSpec {
                tv_sec: inode.ctime as i64,
                tv_nsec: ctime_nsec,
            },
            inode_type,
            mode: FileMode::from_bits_truncate(mode as u32),
            nlinks: inode.links_count as usize,
            uid: inode.uid as u32,
            gid: inode.gid as u32,
            rdev: 0,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let fs = self.fs.lock();
        fs.read_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type == InodeType::Directory {
            return Err(FsError::IsDirectory);
        }

        let fs = self.fs.lock();
        fs.write_at(self.ino, offset, buf)
            .map_err(|_| FsError::IoError)
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let mut fs = self.fs.lock();
        let mut parent = self.ino;
        let mut name_off = 0;

        let child_ino = fs
            .generic_open(name, &mut parent, false, 0, &mut name_off)
            .map_err(|_| FsError::NotFound)?;

        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), child_ino)))
    }

    fn create(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        let fs = self.fs.lock();
        let ftype = ext4_rs::InodeFileType::S_IFREG.bits() | 0o777;

        let child_inode = fs
            .create(self.ino, name, ftype)
            .map_err(|_| FsError::IoError)?;

        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            child_inode.inode_num,
        )))
    }

    fn mkdir(&self, name: &str, _mode: FileMode) -> Result<Arc<dyn Inode>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if self.lookup(name).is_ok() {
            return Err(FsError::AlreadyExists);
        }

        let fs = self.fs.lock();
        let ftype = ext4_rs::InodeFileType::S_IFDIR.bits() | 0o755;

        let mut parent = self.ino;
        let mut name_off = 0;

        let inode_id = fs
            .generic_open(name, &mut parent, true, ftype, &mut name_off)
            .map_err(|_| FsError::NoSpace)?;

        Ok(Arc::new(Ext4Inode::new(self.fs.clone(), inode_id)))
    }

    fn symlink(&self, name: &str, target: &str) -> Result<Arc<dyn Inode>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let parent = self.ino;
        let inode_mod = InodeFileType::S_IFLNK.bits() | 0o777;
        let fs = self.fs.lock();

        let new_inode = fs
            .create(parent, name, inode_mod)
            .map_err(|_| FsError::NoSpace)?;

        fs.write_at(new_inode.inode_num, 0, target.as_bytes())
            .map_err(|_| FsError::IoError)?;

        Ok(Arc::new(Ext4Inode::new(
            self.fs.clone(),
            new_inode.inode_num,
        )))
    }

    fn link(&self, name: &str, target: &Arc<dyn Inode>) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let ext4_inode = target
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        if !Arc::ptr_eq(&self.fs, &ext4_inode.fs) {
            return Err(FsError::InvalidArgument);
        }

        let fs = self.fs.lock();
        let mut self_ref = fs.get_inode_ref(self.ino);
        let mut target_ref = fs.get_inode_ref(ext4_inode.ino);
        fs.link(&mut self_ref, &mut target_ref, name)
            .map_err(|_| FsError::NoSpace)?;

        Ok(())
    }

    fn unlink(&self, name: &str) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let child = self.lookup(name)?;
        let child_metadata = child.metadata()?;

        let child_ext4 = child
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        let fs = self.fs.lock();

        if child_metadata.inode_type == InodeType::Directory {
            fs.dir_remove(self.ino, name)
                .map_err(|_| FsError::IoError)?;
        } else {
            let mut parent_ref = fs.get_inode_ref(self.ino);
            let mut child_ref = fs.get_inode_ref(child_ext4.ino);

            fs.unlink(&mut parent_ref, &mut child_ref, name)
                .map_err(|_| FsError::IoError)?;

            fs.write_back_inode(&mut parent_ref);
        }

        Ok(())
    }

    fn rmdir(&self, name: &str) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let fs = self.fs.lock();
        let parent = self.ino;

        fs.dir_remove(parent, name)
            .map(|_| ())
            .map_err(|_| FsError::NotFound)
    }

    fn rename(
        &self,
        old_name: &str,
        new_parent: Arc<dyn Inode>,
        new_name: &str,
    ) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let old_child = self.lookup(old_name)?;
        let old_child_metadata = old_child.metadata()?;
        let old_child_ext4 = old_child
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        let new_parent_ext4 = new_parent
            .as_any()
            .downcast_ref::<Ext4Inode>()
            .ok_or(FsError::InvalidArgument)?;

        let new_parent_metadata = new_parent_ext4.metadata()?;
        if new_parent_metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        if !Arc::ptr_eq(&self.fs, &new_parent_ext4.fs) {
            return Err(FsError::InvalidArgument);
        }

        if old_child_metadata.inode_type == InodeType::Directory {
            if old_child_ext4.ino == new_parent_ext4.ino {
                return Err(FsError::InvalidArgument);
            }
        }

        let fs = self.fs.lock();

        let mut replaced_inode: Option<u32> = None;

        let target_exists = {
            let mut parent = new_parent_ext4.ino;
            let mut name_off = 0;
            fs.generic_open(new_name, &mut parent, false, 0, &mut name_off)
                .ok()
        };

        if let Some(existing_ino) = target_exists {
            let existing_ref = fs.get_inode_ref(existing_ino);
            let replaced_is_dir = existing_ref.inode.is_dir();

            if replaced_is_dir {
                if fs.dir_has_entry(existing_ino) {
                    return Err(FsError::DirectoryNotEmpty);
                }

                fs.dir_remove(new_parent_ext4.ino, new_name)
                    .map_err(|_| FsError::IoError)?;
            } else {
                let mut new_parent_ref = fs.get_inode_ref(new_parent_ext4.ino);
                let mut existing_ref = fs.get_inode_ref(existing_ino);

                fs.unlink(&mut new_parent_ref, &mut existing_ref, new_name)
                    .map_err(|_| FsError::IoError)?;

                fs.write_back_inode(&mut new_parent_ref);
            }

            replaced_inode = Some(existing_ino);
        }

        let mut old_parent_ref = fs.get_inode_ref(self.ino);
        let mut new_parent_ref = fs.get_inode_ref(new_parent_ext4.ino);
        let child_ref = fs.get_inode_ref(old_child_ext4.ino);

        if let Err(_e) = fs.dir_add_entry(&mut new_parent_ref, &child_ref, new_name) {
            if let Some(replaced_ino) = replaced_inode {
                let replaced_ref = fs.get_inode_ref(replaced_ino);
                let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                fs.write_back_inode(&mut new_parent_ref);
            }
            return Err(FsError::NoSpace);
        }

        if let Err(_e) = fs.dir_remove_entry(&mut old_parent_ref, old_name) {
            let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

            if let Some(replaced_ino) = replaced_inode {
                let replaced_ref = fs.get_inode_ref(replaced_ino);
                let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
            }

            fs.write_back_inode(&mut old_parent_ref);
            fs.write_back_inode(&mut new_parent_ref);
            return Err(FsError::IoError);
        }

        if old_child_metadata.inode_type == InodeType::Directory && self.ino != new_parent_ext4.ino
        {
            let mut child_ref = fs.get_inode_ref(old_child_ext4.ino);

            if let Err(_e) = fs.dir_remove_entry(&mut child_ref, "..") {
                let _ = fs.dir_add_entry(&mut old_parent_ref, &child_ref, old_name);
                let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

                if let Some(replaced_ino) = replaced_inode {
                    let replaced_ref = fs.get_inode_ref(replaced_ino);
                    let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                }

                fs.write_back_inode(&mut old_parent_ref);
                fs.write_back_inode(&mut new_parent_ref);
                fs.write_back_inode(&mut child_ref);
                return Err(FsError::IoError);
            }

            if let Err(_e) = fs.dir_add_entry(&mut child_ref, &new_parent_ref, "..") {
                let _ = fs.dir_add_entry(&mut child_ref, &old_parent_ref, "..");
                let _ = fs.dir_add_entry(&mut old_parent_ref, &child_ref, old_name);
                let _ = fs.dir_remove_entry(&mut new_parent_ref, new_name);

                if let Some(replaced_ino) = replaced_inode {
                    let replaced_ref = fs.get_inode_ref(replaced_ino);
                    let _ = fs.dir_add_entry(&mut new_parent_ref, &replaced_ref, new_name);
                }

                fs.write_back_inode(&mut old_parent_ref);
                fs.write_back_inode(&mut new_parent_ref);
                fs.write_back_inode(&mut child_ref);
                return Err(FsError::NoSpace);
            }

            let old_parent_links = old_parent_ref.inode.links_count();
            if old_parent_links > 0 {
                old_parent_ref.inode.set_links_count(old_parent_links - 1);
            }

            let new_parent_links = new_parent_ref.inode.links_count();
            new_parent_ref.inode.set_links_count(new_parent_links + 1);

            fs.write_back_inode(&mut child_ref);
        }

        fs.write_back_inode(&mut old_parent_ref);
        fs.write_back_inode(&mut new_parent_ref);

        Ok(())
    }

    fn readdir(&self) -> Result<Vec<DirEntry>, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Directory {
            return Err(FsError::NotDirectory);
        }

        let fs = self.fs.lock();

        let entries = fs.dir_get_entries(self.ino);

        let vfs_entries = entries
            .iter()
            .map(|e| {
                let name_len = e.name_len as usize;
                let name = String::from_utf8_lossy(&e.name[..name_len]).into_owned();

                let inode_type = unsafe { Self::convert_dir_entry_type(e.inner.inode_type) };

                DirEntry {
                    name,
                    inode_type,
                    inode_no: e.inode as usize,
                }
            })
            .collect();

        Ok(vfs_entries)
    }

    fn truncate(&self, size: usize) -> Result<(), FsError> {
        let metadata = self.metadata()?;
        let old_size = metadata.size;

        if size == old_size {
            return Ok(());
        }

        if size < old_size {
            let fs = self.fs.lock();
            let mut inode_ref = fs.get_inode_ref(self.ino);
            fs.truncate_inode(&mut inode_ref, size as u64)
                .map_err(|_| FsError::IoError)?;
        } else {
            let extend_size = size - old_size;
            let zero_buf = alloc::vec![0u8; extend_size.min(4096)];

            let fs = self.fs.lock();
            let mut written = 0;
            while written < extend_size {
                let to_write = (extend_size - written).min(zero_buf.len());
                fs.write_at(self.ino, old_size + written, &zero_buf[..to_write])
                    .map_err(|_| FsError::IoError)?;
                written += to_write;
            }
        }

        Ok(())
    }

    fn sync(&self) -> Result<(), FsError> {
        Ok(())
    }

    fn set_dentry(&self, dentry: Weak<Dentry>) {
        *self.dentry.lock() = dentry;
    }

    fn get_dentry(&self) -> Option<Arc<Dentry>> {
        self.dentry.lock().upgrade()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn set_times(&self, atime: Option<TimeSpec>, mtime: Option<TimeSpec>) -> Result<(), FsError> {
        let mut fs = self.fs.lock();

        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        if let Some(at) = atime {
            inode.atime = at.tv_sec as u32;
            inode.i_atime_extra = ((at.tv_nsec as u32) << 2) & 0xFFFFFFFC;
        }

        if let Some(mt) = mtime {
            inode.mtime = mt.tv_sec as u32;
            inode.i_mtime_extra = ((mt.tv_nsec as u32) << 2) & 0xFFFFFFFC;

            let now = fs_ops().timespec_now();
            inode.ctime = now.tv_sec as u32;
            inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;
        }

        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn chown(&self, uid: u32, gid: u32) -> Result<(), FsError> {
        let mut fs = self.fs.lock();

        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        if uid != u32::MAX {
            inode.uid = uid as u16;
        }
        if gid != u32::MAX {
            inode.gid = gid as u16;
        }

        let now = fs_ops().timespec_now();
        inode.ctime = now.tv_sec as u32;
        inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;

        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn chmod(&self, mode: FileMode) -> Result<(), FsError> {
        let mut fs = self.fs.lock();

        let mut inode_ref = fs.get_inode_ref(self.ino);
        let inode = &mut inode_ref.inode;

        let file_type = inode.mode & 0xF000;
        let permission_bits = (mode.bits() & 0x0FFF) as u16;
        inode.mode = file_type | permission_bits;

        let now = fs_ops().timespec_now();
        inode.ctime = now.tv_sec as u32;
        inode.i_ctime_extra = ((now.tv_nsec as u32) << 2) & 0xFFFFFFFC;

        fs.write_back_inode(&mut inode_ref);

        Ok(())
    }

    fn readlink(&self) -> Result<String, FsError> {
        let metadata = self.metadata()?;
        if metadata.inode_type != InodeType::Symlink {
            return Err(FsError::InvalidArgument);
        }

        let size = metadata.size;
        if size == 0 {
            return Ok(String::new());
        }

        let fs = self.fs.lock();
        let mut buf = alloc::vec![0u8; size];

        let bytes_read = fs
            .read_at(self.ino, 0, &mut buf)
            .map_err(|_| FsError::IoError)?;

        buf.truncate(bytes_read);

        String::from_utf8(buf).map_err(|_| FsError::InvalidArgument)
    }

    fn mknod(&self, _name: &str, _mode: FileMode, _dev: u64) -> Result<Arc<dyn Inode>, FsError> {
        Err(FsError::NotSupported)
    }
}
