# Slurmer

[English](README.md) | **简体中文**

一个轻巧友好的终端界面，帮助你在 HPC 集群上监控和管理 SLURM 作业。

**版本：0.5.0**

## ✨ 实用功能

| 功能 | 可以做什么 | 快捷键 |
| --- | --- | --- |
| 实时监控 | 查看自己的作业，自动刷新，默认间隔 10 秒 | `r` 手动刷新 |
| 搜索与筛选 | 模糊搜索作业 ID、名称、用户、分区、QoS 和节点；支持多项筛选，以及名称和节点的正则匹配 | `/` 搜索 · `f` 筛选 |
| 实时日志 | 跟踪 stdout/stderr，暂停浏览历史内容，再恢复跟踪 | `v` |
| 作业脚本 | 查看作业脚本，支持自动换行和翻页 | `Enter` |
| 历史作业 | 查看最近 1、7 或 30 天的记账记录，按状态筛选或搜索 | `h` |
| 完成邮件 | 被监控的作业或整个作业数组结束后，收到一封汇总邮件 | `n` 启用 · `Shift+n` 查看状态 |
| 批量取消 | 选择多个作业，确认后统一取消 | `Space` 选择 · `x` 取消 |
| 自定义列表 | 选择、调整显示列顺序，设置多字段排序 | `c` |
| 配色主题 | 预览 Orange Cream、Sakura Cream、Dark Neon 和 Classic | `s` |

## 🚀 安装

请在**能够访问 SLURM 的 Linux HPC 登录节点**上运行 Slurmer。

需要准备：

- **Git、稳定版 Rust/Cargo**，以及可用的 C 编译器和链接器。可使用集群提供的 Rust 工具链，或参考 [Rust 官方安装指南](https://rust-lang.org/tools/install/)。
- **SLURM 命令：**`squeue`、`sacct`、`sinfo`、`sacctmgr`、`scontrol` 和 `scancel`。
- **仅邮件功能需要：**可正常投递邮件的 `sendmail` 或 `mail`，以及支持 `sacct --array` 的 SLURM 版本。

### 安装并启动

```bash
git clone https://github.com/shiyuzhang0522/Slurmer.git
cd Slurmer
cargo install --path . --locked
```

Cargo 会编译优化后的可执行文件，默认安装到 `~/.cargo/bin`；若设置了
`CARGO_HOME`，则安装到 `$CARGO_HOME/bin`。将该目录加入当前终端的 `PATH`：

```bash
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
```

若希望后续登录也能直接使用，请将同一行配置加入 **Bash 的 `~/.bashrc`**
或 **Zsh 的 `~/.zshrc`**，然后打开新终端，或用 `source` 加载对应文件。

验证安装并启动：

```bash
slurmer --help
slurmer
```

### 更新

在克隆得到的 `Slurmer` 目录中执行：

```bash
git pull --ff-only
cargo install --path . --locked --force
```

重新启动 Slurmer，即可使用更新后的程序。

## ⌨️ 操作指南

在主作业列表中使用上方功能表中的快捷键。其他常用操作：

| 按键 | 操作 |
| --- | --- |
| `↑` / `↓` | 切换作业或滚动文本 |
| `Page Up` / `Page Down` | 按当前可见区域翻页 |
| `Ctrl+u` / `Ctrl+d` | 在作业、脚本、日志和历史视图中翻页 |
| `Shift+↑` / `Shift+↓` | 查看脚本或日志时切换作业 |
| `Space` | 选中或取消选中当前高亮作业 |
| `a` | 全选或取消全选当前显示的作业 |
| `Esc` | 清除正在输入的搜索、关闭弹窗，或从主列表退出 |

- **日志：**`o` 切换 stdout/stderr；向上滚动会暂停跟踪；`End` 恢复 LIVE 实时跟踪。
- **历史：**`f` 切换状态筛选，`t` 切换 1/7/30 天范围，`/` 搜索，`v` 打开日志，`r` 刷新。
- **设置：**`←` / `→` 切换设置项，`↑` / `↓` 选择选项，`Enter` 应用并保存。

查看脚本和日志需要有对应文件的读取权限。历史作业的日志还要求
`scontrol` 能够获取该作业的元数据。

## 📬 完成邮件

**启用通知前，请先设置自己的收件地址**，再启动 Slurmer：

```bash
export SLURMER_EMAIL="you@example.com"
slurmer
```

若未设置，程序内置的收件地址为 **shiyuzhang0522@gmail.com**。

高亮选中数组中的任意子任务，例如 `12345_7`，按 **`n`** 即可监控整个父作业数组
**`12345`**，包括被筛选条件隐藏的任务。普通作业也支持此功能。按 **`Shift+n`**
查看通知状态和错误。每个父作业需要手动启用监控；在同一次界面会话中重复按 `n`
不会创建重复监控。

Slurmer 每 **30 秒**检查一次，等待完成状态的记账记录稳定后发送汇总邮件，内容包括
各状态的任务数量，以及最多 50 个未成功任务的 ID 和退出码。“结束”也包括失败和取消，
并不等于全部成功；记账数据延迟可能导致邮件晚些发送。

### 断开连接后继续监控

**退出 Slurmer 后，界面内启用的监控会停止，重启也不会恢复。** 如需独立于界面和
SSH 会话运行，可在 HPC 上执行以下命令，前提是集群允许后台监控。将 `12345`
替换为自己的作业或数组 ID；后台进程会继承上方设置的 `SLURMER_EMAIL`。

```bash
nohup slurmer --watch 12345 > slurmer-watch-12345.log 2>&1 < /dev/null &
```

监控进程发送一封汇总邮件后自动退出。可查看日志了解状态，或终止该进程以停止监控。
**每个父作业只运行一个监控进程：**不同进程及重启后的监控不共享发送记录，可能重复发信。
从界面监控切换为独立监控前，请先退出界面，停止其中的监控。

### 邮件投递

Slurmer 优先使用 `sendmail`，仅在找不到它时改用 `mail`。
HPC 本身需要已配置好外部邮件投递，无需提供 Gmail 密码。
本地邮件程序接受邮件不代表邮件一定已送达收件箱；未收到时，请检查垃圾邮件或联系
HPC 管理员。SLURM 查询出错会自动重试；邮件投递出错则停止监控。
再次尝试前，请先检查本地邮件队列，避免重复发送。

## 🎨 偏好设置

按 **`s`** 预览主题并调整刷新间隔。新配置默认使用 **Orange Cream**，
已有配置会保留原来保存的主题。Slurmer 默认只显示当前用户的作业，
并自动获取可用分区和 QoS。

主题、刷新间隔、显示列及排序设置会自动保存：

- **Linux/macOS：**`$XDG_CONFIG_HOME/slurmer/config.toml`；未设置该环境变量时，使用 `~/.config/slurmer/config.toml`。
- **Windows：**`%APPDATA%\slurmer\config.toml`。

筛选条件和搜索内容仅在当前会话中保留。邮件收件地址通过 `SLURMER_EMAIL`
设置，不写入偏好配置文件。

## 许可与致谢

本项目使用 [MIT 许可证](LICENSE)。本分支由 Shelley 维护，基于
[wjwei-handsome/Slurmer](https://github.com/wjwei-handsome/Slurmer) 开发。
原项目版权归 wjwei-handsome（<weiwenjie@westlake.edu.cn>）所有。
