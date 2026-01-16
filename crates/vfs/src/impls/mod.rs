//! 文件类型实现

mod blk_dev_file;
mod char_dev_file;
mod pipe_file;
mod reg_file;
mod stdio_file;

pub use blk_dev_file::BlkDeviceFile;
pub use char_dev_file::CharDeviceFile;
pub use pipe_file::PipeFile;
pub use reg_file::RegFile;
pub use stdio_file::{StderrFile, StdinFile, StdoutFile, create_stdio_files};
