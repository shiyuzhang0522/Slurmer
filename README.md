# Slurmer

**English** | [简体中文](README.zh-CN.md)

A friendly terminal interface for monitoring and managing SLURM jobs on an HPC cluster.

**Version: 0.5.0**

## ✨ Useful features

| Feature | What you can do | Shortcut |
| --- | --- | --- |
| Live monitoring | View your jobs with automatic refresh (default: 10 seconds) | `r` refresh |
| Search & filters | Fuzzy-search IDs, names, users, partitions, QoS and nodes; filter jobs, including name/node regex | `/` search · `f` filters |
| Live logs | Follow stdout/stderr, pause to browse, and resume following | `v` |
| Job scripts | Read the job's script with wrapping and paging | `Enter` |
| Job history | Explore accounting records over 1, 7 or 30 days, with state filters and search | `h` |
| Completion emails | Receive one summary when a watched job or entire array finishes | `n` arm · `Shift+n` status |
| Batch cancellation | Select and cancel multiple jobs after confirmation | `Space` select · `x` cancel |
| Customizable table | Choose and reorder columns; sort by multiple fields | `c` |
| Color themes | Preview Orange Cream, Sakura Cream, Dark Neon and Classic | `s` |

## 🚀 Installation

Run Slurmer on a **Linux HPC login node with access to SLURM**.

You need:

- **Git and stable Rust/Cargo**, plus a working C compiler/linker. Use your cluster's Rust toolchain or follow the [official Rust installation guide](https://rust-lang.org/tools/install/).
- **SLURM commands:** `squeue`, `sacct`, `sinfo`, `sacctmgr`, `scontrol` and `scancel`.
- **For email only:** working `sendmail` or `mail` delivery and a SLURM version supporting `sacct --array`.

### Install and launch

```bash
git clone https://github.com/shiyuzhang0522/Slurmer.git
cd Slurmer
cargo install --path . --locked
```

Cargo builds an optimized executable and installs it to `~/.cargo/bin` by default
(or `$CARGO_HOME/bin` if configured). Add that directory to your current shell's `PATH`:

```bash
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
```

For future sessions, add the same line to **`~/.bashrc` for Bash** or
**`~/.zshrc` for Zsh**, then open a new shell or source that file.

Verify and launch:

```bash
slurmer --help
slurmer
```

### Update

From your cloned `Slurmer` directory:

```bash
git pull --ff-only
cargo install --path . --locked --force
```

Restart Slurmer to use the updated executable.

## ⌨️ Controls

Use the feature shortcuts above from the main job list. Additional controls:

| Key | Action |
| --- | --- |
| `↑` / `↓` | Move through jobs or scroll text |
| `Page Up` / `Page Down` | Move by one visible page |
| `Ctrl+u` / `Ctrl+d` | Alternative paging in jobs, scripts, logs and history |
| `Shift+↑` / `Shift+↓` | Switch jobs while viewing scripts or logs |
| `Space` | Select/deselect the highlighted job |
| `a` | Select/deselect all displayed jobs |
| `Esc` | Clear an active search, close a popup, or quit from the main list |

- **Logs:** `o` switches stdout/stderr; scrolling up pauses following; `End` resumes LIVE mode.
- **History:** `f` cycles states, `t` cycles 1/7/30-day windows, `/` searches, `v` opens logs and `r` refreshes.
- **Settings:** `←` / `→` switches fields, `↑` / `↓` chooses an option, `Enter` applies and saves.

Script and log viewing requires readable files. Historical logs also depend on
job metadata still being available through `scontrol`.

## 📬 Completion emails

**Set your recipient before enabling notifications**, then launch Slurmer:

```bash
export SLURMER_EMAIL="you@example.com"
slurmer
```

If unset, the built-in recipient is **shiyuzhang0522@gmail.com**.

Highlight any array task, such as `12345_7`, and press **`n`** to watch the entire
parent array **`12345`**, including tasks hidden by filters. Ordinary jobs work
too. Press **`Shift+n`** to see notification status and errors. Watches are
opt-in per parent job; pressing `n` again does not add a duplicate in that UI session.

Slurmer checks every **30 seconds** and waits for stable completion records
before sending a summary of task states and up to 50 unsuccessful task IDs with
exit codes. “Finished” includes failures and cancellations, not just success;
accounting delays can postpone the email.

### Keep watching after disconnecting

**UI watches stop when Slurmer exits and are not restored on restart.** To watch
independently of the UI or SSH session, use the following on the HPC, if your
site permits background monitoring. Replace `12345` with your job or array ID;
the process inherits `SLURMER_EMAIL` set above.

```bash
nohup slurmer --watch 12345 > slurmer-watch-12345.log 2>&1 < /dev/null &
```

The watcher sends one summary and exits. Read the log for status; terminate the
process to stop watching. Run **one watcher per parent job**: separate processes
and restarted watchers do not share delivery history and can send duplicates.
Exit the UI to stop its watches before switching to standalone monitoring.

### Mail delivery

Slurmer uses `sendmail`, falling back to `mail` only when sendmail is absent.
Your HPC must already support external mail delivery; no Gmail password is needed.
Mailer acceptance does not guarantee inbox delivery—check spam or contact your
HPC administrator if mail does not arrive. SLURM query errors are retried;
mail delivery errors stop the watch. Check the local mail queue before retrying
to avoid duplicate messages.

## 🎨 Preferences

Press **`s`** to preview themes and set the refresh interval. **Orange Cream** is
the default for new configurations; existing saved theme choices are preserved.
Slurmer defaults to your current username and discovers available partitions and QoS values.

Theme, refresh interval, columns and sort order are saved automatically:

- **Linux/macOS:** `$XDG_CONFIG_HOME/slurmer/config.toml`, or `~/.config/slurmer/config.toml` when unset.
- **Windows:** `%APPDATA%\slurmer\config.toml`.

Filters and search queries last only for the current session. The email recipient
is configured through `SLURMER_EMAIL`, not the preferences file.

## License and attribution

Licensed under the [MIT License](LICENSE). This fork is maintained by Shelley
and based on [wjwei-handsome/Slurmer](https://github.com/wjwei-handsome/Slurmer).
Original copyright © wjwei-handsome (<weiwenjie@westlake.edu.cn>).
