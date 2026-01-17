//! `/proc/[pid]` 目录下的进程级文件生成器集合

pub mod cmdline;
pub mod maps;
pub mod stat;
pub mod status;

pub use cmdline::CmdlineGenerator;
pub use maps::MapsGenerator;
pub use stat::StatGenerator;
pub use status::StatusGenerator;
