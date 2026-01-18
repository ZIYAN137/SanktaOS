//! Build script for the OS kernel
//!
//! This script automatically creates ext4 filesystem image

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Switching between `cargo build` and `make test` toggles TEST=1.
    // Make sure Cargo re-runs this build script when that env changes,
    // otherwise EXT4_FS_IMAGE may keep pointing at the dummy image.
    println!("cargo:rerun-if-env-changed=TEST");

    // 获取环境变量
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    // 设置路径
    let project_root = PathBuf::from(&manifest_dir).parent().unwrap().to_path_buf();

    // 步骤 1: 创建 ext4 镜像
    // 检测是否为测试模式
    // 注意: CARGO_CFG_TEST 只在运行测试时设置，编译时不会设置
    // 因此我们检查 TEST 环境变量 (由 Makefile 传递) 或检查是否有测试相关的 cfg
    let is_test = env::var("TEST").is_ok()
        || env::var("CARGO_CFG_TEST").is_ok()
        || env::var("PROFILE").map(|p| p == "test").unwrap_or(false);

    // 1.1: 创建用于 include_bytes! 嵌入的镜像
    let ext4_embed_img = PathBuf::from(&out_dir).join("ext4_test.img");
    if is_test {
        // 测试模式: 创建 8MB 镜像用于测试
        // 只有在测试模式下才需要这个环境变量
        println!("cargo:warning=[build.rs] Creating ext4 test image for embedding (8MB)...");
        create_ext4_test_image(&ext4_embed_img);
        println!("cargo:rustc-env=EXT4_FS_IMAGE={}", ext4_embed_img.display());
    } else {
        // IDE 修复: 即使不在测试模式下，也需要定义 EXT4_FS_IMAGE 环境变量
        // 这里的代码会被 rust-analyzer 分析，如果缺少环境变量会报错
        // 我们创建一个空的伪文件来满足 include_bytes! 的需求
        let dummy_img = PathBuf::from(&out_dir).join("ext4_test_dummy.img");
        if !dummy_img.exists() {
            let _ = fs::write(&dummy_img, &[]);
        }
        println!(
            "cargo:warning=[build.rs] Skipping real test image creation (using dummy for IDE)"
        );
        println!("cargo:rustc-env=EXT4_FS_IMAGE={}", dummy_img.display());
    }

    // 1.2: 非测试模式下创建完整的运行时镜像
    if !is_test {
        let target = env::var("TARGET").unwrap_or_default();
        let arch_key = match env::var("ARCH") {
            Ok(val) if !val.is_empty() => {
                if val.contains("loongarch") {
                    "loongarch"
                } else {
                    "riscv"
                }
            }
            _ => {
                if target.contains("loongarch") {
                    "loongarch"
                } else {
                    "riscv"
                }
            }
        };
        let fs_img_name = format!("fs-{}.img", arch_key);
        let fs_img_path = PathBuf::from(&manifest_dir).join(&fs_img_name);
        let data_dir = if arch_key == "loongarch" {
            project_root.join("data").join("loongarch_musl")
        } else {
            project_root.join("data").join("risc-v_musl")
        };
        // NOTE: OUT_DIR changes across profiles/features, so this stamp may be missing even if the
        // already-built fs image is still up-to-date. Missing stamp must NOT force a rebuild.
        let arch_stamp = PathBuf::from(&out_dir).join(format!("fs_img_arch_{}.txt", arch_key));

        // 检查依赖
        println!("cargo:rerun-if-changed={}", data_dir.display());
        let dependencies = vec![data_dir.clone()];

        let saved_arch = fs::read_to_string(&arch_stamp).ok();
        let force_rebuild = saved_arch
            .as_deref()
            .map(|s| s.trim() != arch_key)
            .unwrap_or(false);
        if force_rebuild {
            println!(
                "cargo:warning=[build.rs] Arch changed ({}), forcing fs.img rebuild.",
                arch_key
            );
        }

        if force_rebuild || should_rebuild(&fs_img_path, &dependencies) {
            println!(
                "cargo:warning=[build.rs] Creating full ext4 runtime image (4GB) at fs.img..."
            );
            create_full_ext4_image(&fs_img_path, &data_dir, &project_root);
            let _ = fs::write(&arch_stamp, format!("{}\n", arch_key));
            println!(
                "cargo:warning=[build.rs] Runtime image created: {}",
                fs_img_path.display()
            );
        } else {
            println!(
                "cargo:warning=[build.rs] {} is up to date, skipping regeneration.",
                fs_img_name
            );
            // Stamp may be missing when OUT_DIR changes across builds; write it to avoid
            // accidental rebuilds next time.
            if saved_arch.is_none() {
                let _ = fs::write(&arch_stamp, format!("{}\n", arch_key));
            }
        }
    }
}

/// 检查目标文件是否需要重新构建
///
/// 如果目标不存在，或者任何依赖项(目录或文件)比目标新，则返回 true
fn should_rebuild(target: &Path, dependencies: &[PathBuf]) -> bool {
    if !target.exists() {
        return true;
    }

    let target_mtime = match fs::metadata(target).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return true, // 无法获取时间，为了安全起见重新构建
    };

    for dep in dependencies {
        if let Some(latest_dep_mtime) = get_latest_mtime(dep) {
            if latest_dep_mtime > target_mtime {
                return true;
            }
        } else {
            return true;
        }
    }

    false
}

/// 递归获取目录中最新的修改时间
fn get_latest_mtime(path: &Path) -> Option<std::time::SystemTime> {
    if !path.exists() {
        return None;
    }

    if path.is_file() {
        return fs::metadata(path).ok().and_then(|m| m.modified().ok());
    }

    let mut latest = None;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 忽略隐藏文件 (.git 等)
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            let mtime = get_latest_mtime(&path);
            match (latest, mtime) {
                (None, Some(m)) => latest = Some(m),
                (Some(l), Some(m)) if m > l => latest = Some(m),
                _ => {}
            }
        }
    }
    latest
}

/// 创建 ext4 测试镜像 (8MB)
fn create_ext4_test_image(path: &PathBuf) {
    create_empty_ext4_image(path, 8);
}

/// 创建空的 ext4 镜像
fn create_empty_ext4_image(path: &PathBuf, size_mb: usize) {
    const BLOCK_SIZE: usize = 1024 * 1024;

    println!(
        "cargo:warning=[build.rs] Creating {}MB ext4 image at {}",
        size_mb,
        path.display()
    );

    // 1. 创建空文件 (dd)
    let dd_status = Command::new("dd")
        .arg("if=/dev/zero")
        .arg(format!("of={}", path.display()))
        .arg(format!("bs={}", BLOCK_SIZE))
        .arg(format!("count={}", size_mb))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute dd");

    if !dd_status.success() {
        panic!("Failed to create empty disk image");
    }

    // 2. 格式化为 ext4
    // 关键: 使用 -g 512 强制生成多个块组以避免 ext4_rs bug
    // 每组 512 块 (2MB), 8MB 镜像 = 4 个块组
    let mut mkfs_cmd = Command::new("mkfs.ext4");
    mkfs_cmd
        .arg("-F") // 强制覆盖
        .arg("-b")
        .arg("4096") // 块大小 4K
        .arg("-m")
        .arg("0") // 0% 保留空间
        .arg("-I")
        .arg("256"); // Inode 大小 256 字节

    // 只对测试镜像 (>=8MB) 添加 -g 选项以生成多个块组
    if size_mb >= 8 {
        mkfs_cmd.arg("-g").arg("512"); // 每组 512 块 (2MB)
    }

    let mkfs_status = mkfs_cmd
        .arg("-O")
        .arg("64bit,^has_journal,^resize_inode,^dir_index,^metadata_csum")
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute mkfs.ext4");

    if mkfs_status.success() {
        let block_groups = if size_mb >= 8 { " (multi-group)" } else { "" };
        println!(
            "cargo:warning=[build.rs] Ext4 image formatted successfully ({}MB{}).",
            size_mb, block_groups
        );
    } else {
        panic!("Failed to format ext4 image! Make sure 'mkfs.ext4' is installed.");
    }
}

/// 创建完整的 ext4 镜像 (包含 data/)
fn create_full_ext4_image(path: &PathBuf, data_dir: &Path, _project_root: &Path) {
    const IMG_SIZE_MB: usize = 4096; // 4GB
    const BLOCK_SIZE: usize = 1024 * 1024;

    println!(
        "cargo:warning=[build.rs] Creating {}MB (4GB) full ext4 image at {}",
        IMG_SIZE_MB,
        path.display()
    );

    // 1. 创建临时目录用于组织文件系统内容
    let temp_root = std::env::temp_dir().join("comix_fs_content");
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root).ok();
    }
    fs::create_dir_all(&temp_root).expect("Failed to create temp directory");

    // 2. 复制 data 目录的内容到临时根目录
    if data_dir.exists() {
        copy_dir_recursive(&data_dir.to_path_buf(), &temp_root)
            .expect("Failed to copy data directory");
        println!(
            "cargo:warning=[build.rs] Copied {} to temp root",
            data_dir.display()
        );
    }

    // 3. 创建空镜像
    let dd_status = Command::new("dd")
        .arg("if=/dev/zero")
        .arg(format!("of={}", path.display()))
        .arg(format!("bs={}", BLOCK_SIZE))
        .arg(format!("count={}", IMG_SIZE_MB))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute dd");

    if !dd_status.success() {
        panic!("Failed to create disk image");
    }

    // 4. 使用 mkfs.ext4 -d 选项从临时目录创建文件系统
    let mkfs_status = Command::new("mkfs.ext4")
        .arg("-F")
        .arg("-b")
        .arg("4096")
        .arg("-m")
        .arg("0")
        .arg("-d")
        .arg(&temp_root) // 使用临时目录作为根
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute mkfs.ext4");

    if !mkfs_status.success() {
        panic!("Failed to format ext4 image with data!");
    }

    // 5. 清理临时目录
    fs::remove_dir_all(&temp_root).ok();

    println!("cargo:warning=[build.rs] Full ext4 image created successfully (1GB).");
}

/// 递归复制目录
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
