# Slurmer

Slurmer is a fast terminal interface for monitoring and managing SLURM jobs on
an HPC cluster.

This fork introduces a cute Sakura Cream interface, fuzzy job search,
viewport-aware paging, safer log rendering, persistent UI preferences, and
reliability improvements. It is based on the
original [wjwei-handsome/Slurmer](https://github.com/wjwei-handsome/Slurmer)
project.

Current version: **0.3.0**

## Features

- Sakura Cream light theme with blush-pink surfaces and lavender borders
- Dark Neon and Classic themes remain available
- Real-time job monitoring with configurable automatic refresh
- Page Up/Page Down navigation in jobs, logs, and scripts
- Fuzzy search across job ID, name, user, partition, QoS, and node
- Filters for user, state, partition, QoS, job name, and node
- Customizable columns and multi-column sorting
- Safely sanitized, soft-wrapped job script and stdout/stderr log viewers
- LIVE log following with PAUSED history browsing and visible page/line ranges
- Select and cancel one or multiple jobs
- Persistent theme, refresh interval, column, and sorting preferences

## Requirements

- A working SLURM installation with `squeue`, `sinfo`, `sacctmgr`, `scontrol`,
  and `scancel`
- A Rust toolchain for building from source

## Build and install on the HPC

```bash
git clone https://github.com/shiyuzhang0522/Slurmer.git
cd Slurmer
cargo build --release
```

The executable is created at:

```text
target/release/slurmer
```

For the current HPC installation, add its release directory to `PATH`:

```bash
echo 'export PATH="/public/home/hpc8301200407/tool/Slurmer/target/release:$PATH"' \
  >> /public/home/hpc8301200407/.bashrc
source /public/home/hpc8301200407/.bashrc
```

Verify and launch:

```bash
which slurmer
slurmer
```

## Keyboard shortcuts

| Key | Action |
|---|---|
| <kbd>↑</kbd>/<kbd>↓</kbd> | Navigate jobs |
| <kbd>Page Up</kbd>/<kbd>Page Down</kbd> | Move by one visible page |
| <kbd>Ctrl+u</kbd>/<kbd>Ctrl+d</kbd> | Alternative page navigation |
| <kbd>Shift</kbd> + <kbd>↑</kbd>/<kbd>↓</kbd> | Change jobs in script/log views |
| <kbd>Space</kbd> | Select or deselect the highlighted job |
| <kbd>a</kbd> | Select or deselect all displayed jobs |
| <kbd>/</kbd> | Fuzzy-search loaded jobs |
| <kbd>f</kbd> | Open job filters |
| <kbd>s</kbd> | Open theme and refresh settings |
| <kbd>c</kbd> | Configure columns and sorting |
| <kbd>Enter</kbd> | View the selected job script |
| <kbd>v</kbd> | View stdout/stderr logs |
| <kbd>End</kbd> | Resume LIVE following in the log viewer |
| <kbd>r</kbd> | Refresh the job list |
| <kbd>x</kbd> | Cancel selected jobs after confirmation |
| <kbd>Esc</kbd> | Clear search, close a popup, or quit |

Additional controls are displayed inside each popup.

## Configuration

Slurmer automatically detects available partitions and QoS values and uses the
current username as its default job filter.

Theme, refresh interval, selected columns, and sort order are saved to:

- Linux/macOS: `$XDG_CONFIG_HOME/slurmer/config.toml` or
  `~/.config/slurmer/config.toml`
- Windows: `%APPDATA%\slurmer\config.toml`

Job filters and fuzzy-search queries remain session-only. Sakura Cream is the
default for new configurations. Existing saved Dark Neon or Classic choices
are preserved, and all themes can be selected by pressing <kbd>s</kbd>.

## Updating the HPC installation

After pulling new changes, rebuild the release executable:

```bash
cd /public/home/hpc8301200407/tool/Slurmer
git pull
cargo build --release
```

Because the release directory is already in `PATH`, the updated executable is
used immediately after a successful rebuild.

## License and attribution

Fork maintained by Shelley. Original project copyright (c) wjwei-handsome
<weiwenjie@westlake.edu.cn>.

This project is licensed under the MIT license ([LICENSE] or
<http://opensource.org/licenses/MIT>).

[LICENSE]: ./LICENSE
