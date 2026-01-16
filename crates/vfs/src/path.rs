//! 路径解析引擎
//!
//! 该模块实现了 VFS 的路径解析功能，负责将路径字符串转换为 Dentry。
//!
//! 支持的典型语义：
//!
//! - 绝对路径以 `/` 开头，从根目录开始解析；相对路径从“当前工作目录”开始解析
//! - `.` 表示当前目录，解析时跳过；`..` 表示父目录（绝对路径不允许越过根）
//! - 支持符号链接解析：`vfs_lookup` 默认跟随；`vfs_lookup_no_follow` 不跟随最后一个组件

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{DENTRY_CACHE, Dentry, FsError, InodeType, MOUNT_TABLE, get_root_dentry, vfs_ops};

const MAX_SYMLINK_DEPTH: usize = 8;

/// 路径组件
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathComponent {
    /// 根目录 "/"
    Root,
    /// 当前目录 "."
    Current,
    /// 父目录 ".."
    Parent,
    /// 正常的文件名
    Normal(String),
}

/// 将路径字符串解析为组件列表
pub fn parse_path(path: &str) -> Vec<PathComponent> {
    let mut components = Vec::new();

    // 绝对路径以 Root 开始
    if path.starts_with('/') {
        components.push(PathComponent::Root);
    }

    // 分割路径并解析每个部分
    for part in path.split('/').filter(|s| !s.is_empty()) {
        let component = match part {
            "." => PathComponent::Current,
            ".." => PathComponent::Parent,
            name => PathComponent::Normal(String::from(name)),
        };
        components.push(component);
    }

    components
}

/// 规范化路径（处理 ".." 和 "."）
pub fn normalize_path(path: &str) -> String {
    let components = parse_path(path);
    let mut stack: Vec<String> = Vec::new();
    let mut is_absolute = false;

    for component in components {
        match component {
            PathComponent::Root => {
                is_absolute = true;
            }
            PathComponent::Current => {
                // "." 不做任何操作
            }
            PathComponent::Parent => {
                if is_absolute {
                    // 绝对路径：不能越过根目录
                    if !stack.is_empty() {
                        stack.pop();
                    }
                } else {
                    // 相对路径：
                    if let Some(last) = stack.last() {
                        if last == ".." {
                            stack.push(String::from(".."));
                        } else {
                            stack.pop();
                        }
                    } else {
                        stack.push(String::from(".."));
                    }
                }
            }
            PathComponent::Normal(name) => {
                stack.push(name);
            }
        }
    }

    // 构造结果
    if stack.is_empty() {
        if is_absolute {
            String::from("/")
        } else {
            String::from(".")
        }
    } else if is_absolute {
        String::from("/") + &stack.join("/")
    } else {
        stack.join("/")
    }
}

/// 将路径分割为目录部分和文件名部分
pub fn split_path(path: &str) -> Result<(String, String), FsError> {
    // 如果路径以斜杠结尾，说明是目录而非文件，返回错误
    if path.ends_with('/') && path.len() > 1 {
        return Err(FsError::InvalidArgument);
    }

    // 先规范化路径
    let normalized = normalize_path(path);

    if let Some(pos) = normalized.rfind('/') {
        let dir = if pos == 0 {
            String::from("/")
        } else {
            String::from(&normalized[..pos])
        };
        let filename = String::from(&normalized[pos + 1..]);

        if filename.is_empty() {
            return Err(FsError::InvalidArgument);
        }

        Ok((dir, filename))
    } else {
        // 相对路径，使用当前目录
        Ok((String::from("."), String::from(normalized)))
    }
}

/// 将路径字符串解析为 Dentry（支持绝对/相对路径、符号链接解析）
pub fn vfs_lookup(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);

    // 确定起始 dentry
    let current_dentry = if components.first() == Some(&PathComponent::Root) {
        // 绝对路径：从根目录开始
        get_root_dentry()?
    } else {
        // 相对路径：从当前工作目录开始
        get_cur_dir()?
    };

    vfs_walk(current_dentry, components, true)
}

/// 从指定的 base dentry 开始解析路径
pub fn vfs_lookup_from(base: Arc<Dentry>, path: &str) -> Result<Arc<Dentry>, FsError> {
    let components: Vec<PathComponent> = parse_path(path)
        .into_iter()
        .filter(|c| *c != PathComponent::Root)
        .collect();
    vfs_walk(base, components, true)
}

/// 解析单个路径组件
fn resolve_component(base: Arc<Dentry>, component: PathComponent) -> Result<Arc<Dentry>, FsError> {
    match component {
        PathComponent::Root => get_root_dentry(),
        PathComponent::Current => Ok(base),
        PathComponent::Parent => {
            match base.parent() {
                Some(parent) => check_mount_point(parent),
                None => Ok(base), // 根目录的父目录是自己
            }
        }
        PathComponent::Normal(name) => {
            // 1. 先检查 dentry 缓存
            if let Some(child) = base.lookup_child(&name) {
                return check_mount_point(child);
            }

            // 2. 缓存未命中，通过 inode 查找
            let child_inode = base.inode.lookup(&name)?;

            // 3. 创建新的 dentry 并加入缓存
            let child_dentry = Dentry::new(name.clone(), child_inode);
            if child_dentry.inode.cacheable() {
                base.add_child(child_dentry.clone());
                DENTRY_CACHE.insert(&child_dentry);
            } else {
                child_dentry.set_parent(&base);
            }

            // 4. 检查是否有挂载点
            check_mount_point(child_dentry)
        }
    }
}

fn vfs_walk(
    mut current_dentry: Arc<Dentry>,
    mut components: Vec<PathComponent>,
    follow_last_symlink: bool,
) -> Result<Arc<Dentry>, FsError> {
    let mut i = 0usize;
    let mut symlink_depth = 0usize;

    while i < components.len() {
        let component = components[i].clone();
        let is_last = i + 1 == components.len();

        current_dentry = resolve_component(current_dentry, component)?;

        let inode_type = current_dentry.inode.metadata()?.inode_type;
        if inode_type == InodeType::Symlink && (follow_last_symlink || !is_last) {
            if symlink_depth >= MAX_SYMLINK_DEPTH {
                return Err(FsError::TooManySymlinks);
            }
            symlink_depth += 1;

            let target = current_dentry.inode.readlink()?;

            current_dentry = if target.starts_with('/') {
                get_root_dentry()?
            } else {
                match current_dentry.parent() {
                    Some(parent) => parent,
                    None => get_root_dentry()?,
                }
            };

            let mut target_components = parse_path(&target);
            let mut remaining = components.split_off(i + 1);
            target_components.append(&mut remaining);
            components = target_components;
            i = 0;
            continue;
        }

        i += 1;
    }

    Ok(current_dentry)
}

/// 检查给定的 dentry 是否有挂载点
fn check_mount_point(dentry: Arc<Dentry>) -> Result<Arc<Dentry>, FsError> {
    // 快速路径：检查 dentry 本地缓存
    if let Some(mounted_root) = dentry.get_mount() {
        return Ok(mounted_root);
    }

    // 慢速路径：查找挂载表
    let full_path = dentry.full_path();
    if let Some(mount_point) = MOUNT_TABLE.find_mount(&full_path) {
        if mount_point.mount_path == full_path {
            dentry.set_mount(&mount_point.root);
            return Ok(mount_point.root.clone());
        }
    }

    Ok(dentry)
}

/// 获取当前任务的工作目录
fn get_cur_dir() -> Result<Arc<Dentry>, FsError> {
    vfs_ops().current_cwd().ok_or(FsError::NotSupported)
}

/// 查找路径但不跟随最后一个符号链接
pub fn vfs_lookup_no_follow(path: &str) -> Result<Arc<Dentry>, FsError> {
    let components = parse_path(path);

    if components.is_empty() {
        return Err(FsError::InvalidArgument);
    }

    // 确定起始 dentry
    let current_dentry = if components.first() == Some(&PathComponent::Root) {
        get_root_dentry()?
    } else {
        get_cur_dir()?
    };

    vfs_walk(current_dentry, components, false)
}

/// 从指定的 base dentry 开始查找路径，但不跟随最后一个符号链接
pub fn vfs_lookup_no_follow_from(base: Arc<Dentry>, path: &str) -> Result<Arc<Dentry>, FsError> {
    let components: Vec<PathComponent> = parse_path(path)
        .into_iter()
        .filter(|c| *c != PathComponent::Root)
        .collect();
    vfs_walk(base, components, false)
}
