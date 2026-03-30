# Development

## 本地

```bash
cargo run
cargo run -- debug
cargo run -- pipe -s 1 --device-info
```

## 发布

打 tag 即可：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 会构建 `x86_64-unknown-linux-musl`，上传 zip、sha256 和 `install.sh`。
