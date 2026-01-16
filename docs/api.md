# API 文档（rustdoc）

本项目的 API 参考以 rustdoc 为准。你可以在本地生成并查看：

```bash
# 生成整个 workspace 的 API 文档（不包含依赖）
cargo doc --workspace --no-deps

# 生成并打开（需要本机可打开浏览器）
cargo doc --workspace --no-deps --open
```

生成结果默认位于：`target/doc/index.html`。

说明：若 workspace 当前存在编译错误，`cargo doc` 可能失败；此时可先对单个 crate 生成文档，例如：

```bash
cargo doc --manifest-path crates/net/Cargo.toml --no-deps
```
