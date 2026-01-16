use super::*;
// P1 重要功能测试

#[test_case]
fn test_create_stdio_files() {
    // 创建标准 I/O 文件
    let (stdin, stdout, stderr) = create_stdio_files();

    // 验证 stdin
    assert!(stdin.readable());
    assert!(!stdin.writable());

    // 验证 stdout
    assert!(!stdout.readable());
    assert!(stdout.writable());

    // 验证 stderr
    assert!(!stderr.readable());
    assert!(stderr.writable());
}
