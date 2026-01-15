//! 块设备文件的 File trait 实现

use alloc::sync::Arc;
use sync::SpinLock;

use crate::devno::get_blkdev_index;
use crate::{device_ops, Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};

/// 块设备文件
pub struct BlkDeviceFile {
    /// 关联的 dentry
    pub dentry: Arc<Dentry>,

    /// 关联的 inode
    pub inode: Arc<dyn Inode>,

    /// 设备号
    dev: u64,

    /// 块设备驱动索引
    blk_index: Option<usize>,

    /// 打开标志位
    pub flags: OpenFlags,

    /// 当前偏移量（字节）
    offset: SpinLock<usize>,
}

impl BlkDeviceFile {
    /// 创建新的块设备文件
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Result<Self, FsError> {
        let inode = dentry.inode.clone();
        let metadata = inode.metadata()?;
        let dev = metadata.rdev;

        let blk_index = get_blkdev_index(dev);

        if blk_index.is_none() {
            return Err(FsError::NoDevice);
        }

        Ok(Self {
            dentry,
            inode,
            dev,
            blk_index,
            flags,
            offset: SpinLock::new(0),
        })
    }

    const BLOCK_SIZE: usize = 512;
}

impl File for BlkDeviceFile {
    fn readable(&self) -> bool {
        self.flags.readable()
    }

    fn writable(&self) -> bool {
        self.flags.writable()
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }

        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;

        let mut offset_guard = self.offset.lock();
        let current_offset = *offset_guard;

        let start_sector = current_offset / Self::BLOCK_SIZE;
        let sector_offset = current_offset % Self::BLOCK_SIZE;

        let mut total_read = 0;
        let mut remaining = buf.len();

        while remaining > 0 {
            let sector_idx = start_sector + total_read / Self::BLOCK_SIZE;
            let offset_in_sector = if total_read == 0 { sector_offset } else { 0 };
            let to_read = remaining.min(Self::BLOCK_SIZE - offset_in_sector);

            let mut sector_buf = [0u8; 512];
            if !device_ops().read_block(blk_idx, sector_idx, &mut sector_buf) {
                return Err(FsError::IoError);
            }

            buf[total_read..total_read + to_read]
                .copy_from_slice(&sector_buf[offset_in_sector..offset_in_sector + to_read]);

            total_read += to_read;
            remaining -= to_read;
        }

        *offset_guard = current_offset + total_read;
        Ok(total_read)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }

        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;

        let mut offset_guard = self.offset.lock();
        let current_offset = *offset_guard;

        let start_sector = current_offset / Self::BLOCK_SIZE;
        let sector_offset = current_offset % Self::BLOCK_SIZE;

        let mut total_written = 0;
        let mut remaining = buf.len();

        while remaining > 0 {
            let sector_idx = start_sector + total_written / Self::BLOCK_SIZE;
            let offset_in_sector = if total_written == 0 { sector_offset } else { 0 };
            let to_write = remaining.min(Self::BLOCK_SIZE - offset_in_sector);

            let mut sector_buf = [0u8; 512];

            if offset_in_sector != 0 || to_write != Self::BLOCK_SIZE {
                if !device_ops().read_block(blk_idx, sector_idx, &mut sector_buf) {
                    return Err(FsError::IoError);
                }
            }

            sector_buf[offset_in_sector..offset_in_sector + to_write]
                .copy_from_slice(&buf[total_written..total_written + to_write]);

            if !device_ops().write_block(blk_idx, sector_idx, &sector_buf) {
                return Err(FsError::IoError);
            }

            total_written += to_write;
            remaining -= to_write;
        }

        *offset_guard = current_offset + total_written;
        Ok(total_written)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        let blk_idx = self.blk_index.ok_or(FsError::NoDevice)?;

        let device_size = device_ops().blkdev_total_blocks(blk_idx) * Self::BLOCK_SIZE;

        let mut offset_guard = self.offset.lock();
        let current = *offset_guard as isize;

        let new_offset = match whence {
            SeekWhence::Set => offset,
            SeekWhence::Cur => current + offset,
            SeekWhence::End => device_size as isize + offset,
        };

        if new_offset < 0 {
            return Err(FsError::InvalidArgument);
        }

        *offset_guard = new_offset as usize;
        Ok(new_offset as usize)
    }

    fn offset(&self) -> usize {
        *self.offset.lock()
    }

    fn flags(&self) -> OpenFlags {
        self.flags.clone()
    }

    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.inode.clone())
    }

    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry.clone())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
