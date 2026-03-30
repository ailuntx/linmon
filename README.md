# linmon

Linux 终端监控工具，支持普通 Linux 和 WSL2。

## 安装

```bash
curl -fsSL https://github.com/ailuntz/linmon/releases/latest/download/install.sh | bash
```

安装后会把 `linmon` 放到 `~/.local/bin`，并补 PATH。新开一个 shell 后直接执行：

```bash
linmon
linmon debug
linmon pipe -s 1 --device-info
```

## 手动安装

下载 release 里的 `linmon-linux-x86_64.zip`，解压后执行：

```bash
./linmon
```

首次运行会把自己复制到 `~/.local/bin/linmon`。

## 说明

- WSL2 的 GPU 走 `nvidia-smi`
- 真正的 Linux 优先走 `cpufreq`、`hwmon`、`powercap`
- 没有传感器的数据会显示为 `N/A` 或 `null`
