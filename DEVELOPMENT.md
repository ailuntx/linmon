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
git tag vX.Y.Z
git push origin vX.Y.Z
```

GitHub Actions 会构建 `x86_64-unknown-linux-musl`，上传 zip、deb、sha256、`install.sh`，并更新 GitHub Pages 上的 apt 源。

## Debian 打包

仓库里已经有 `debian/`。

本地或容器里可以直接：

```bash
dpkg-buildpackage -us -uc -b
scripts/build-debian-source.sh
```
