//! Opt-in, array-wide completion mail. All SLURM and mail I/O runs off the UI thread.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    process::{Command, Output, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use color_eyre::{eyre::eyre, Result};

pub const DEFAULT_EMAIL: &str = "shiyuzhang0522@gmail.com";
const POLL_INTERVAL: Duration = Duration::from_secs(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OUTPUT: u64 = 16 * 1024 * 1024;

pub fn recipient() -> Result<String> {
    let email = std::env::var("SLURMER_EMAIL").unwrap_or_else(|_| DEFAULT_EMAIL.into());
    validate_email(&email)?;
    Ok(email)
}

fn validate_email(email: &str) -> Result<()> {
    if !email
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"@._+-".contains(&c))
        || email.starts_with('-')
        || email.chars().any(|c| c.is_whitespace() || c.is_control())
        || email.matches('@').count() != 1
        || email.split('@').any(str::is_empty)
    {
        return Err(eyre!("Invalid notification email address"));
    }
    Ok(())
}

/// `123_7` and `123_[7-20%4]` refer to the same parent, 123.
/// Reject steps, heterogeneous IDs and arbitrary command arguments.
pub fn parent_id(id: &str) -> Result<String> {
    let (parent, suffix) = id.split_once('_').unwrap_or((id, ""));
    if parent.is_empty()
        || !parent.bytes().all(|c| c.is_ascii_digit())
        || (id.contains('_')
            && (suffix.is_empty()
                || !suffix
                    .bytes()
                    .all(|c| c.is_ascii_digit() || b"[],-:%".contains(&c))))
    {
        return Err(eyre!("Unsupported job ID: {id}"));
    }
    Ok(parent.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskResult {
    state: String,
    exit_code: String,
}

type Records = BTreeMap<String, TaskResult>;

fn terminal(state: &str) -> bool {
    matches!(
        state,
        "COMPLETED"
            | "FAILED"
            | "CANCELLED"
            | "TIMEOUT"
            | "NODE_FAIL"
            | "OUT_OF_MEMORY"
            | "BOOT_FAIL"
            | "DEADLINE"
            | "REVOKED"
            | "PREEMPTED"
    )
}

fn accounting_records(text: &str, parent: &str) -> Result<Records> {
    let mut records = Records::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let fields: Vec<_> = line.split('|').map(str::trim).collect();
        if fields.len() < 3 {
            return Err(eyre!("Incomplete sacct output; waiting for accounting"));
        }
        let id = fields[0];
        if id.contains('.') {
            continue; // Batch/extern steps are not array tasks.
        }
        if parent_id(id).ok().as_deref() != Some(parent) {
            continue;
        }
        // Only concrete task IDs are accepted; a compressed range is not a result.
        if id.contains('_')
            && !id
                .split_once('_')
                .unwrap()
                .1
                .bytes()
                .all(|c| c.is_ascii_digit())
        {
            return Err(eyre!("Accounting returned an unexpanded array range"));
        }
        let state = fields[1]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        records.insert(
            id.into(),
            TaskResult {
                state,
                exit_code: fields[2].into(),
            },
        );
    }
    Ok(records)
}

#[derive(Default)]
struct CompletionTracker {
    seen: BTreeSet<String>,
    candidate: Option<Records>,
}

impl CompletionTracker {
    /// Require two identical complete snapshots, with no queued tasks in either.
    /// Never infer success from disappearance, missing accounting or command errors.
    fn observe(&mut self, active: &[String], records: Records) -> Option<Records> {
        self.seen.extend(active.iter().cloned());
        self.seen.extend(records.keys().cloned());
        let ready = active.is_empty()
            && !records.is_empty()
            && self.seen.iter().all(|id| records.contains_key(id))
            && records.values().all(|row| terminal(&row.state));
        if !ready {
            self.candidate = None;
            return None;
        }
        if self.candidate.as_ref() == Some(&records) {
            return Some(records);
        }
        self.candidate = Some(records);
        None
    }
}

fn capture(command: Command, input: Option<Vec<u8>>) -> Result<Output> {
    capture_with_timeout(command, input, COMMAND_TIMEOUT)
}

fn capture_with_timeout(
    mut command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
) -> Result<Output> {
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |stream: Box<dyn Read + Send>| {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = (|| -> io::Result<Vec<u8>> {
                let mut bytes = Vec::new();
                stream.take(MAX_OUTPUT + 1).read_to_end(&mut bytes)?;
                if bytes.len() as u64 > MAX_OUTPUT {
                    return Err(io::Error::other("Command output exceeds 16 MiB"));
                }
                Ok(bytes)
            })();
            let _ = sender.send(result);
        });
        receiver
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(stdin.write_all(&bytes));
        });
        receiver
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(eyre!(
                "Command timed out after {} seconds",
                timeout.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let output = Output {
        status,
        stdout: out
            .recv_timeout(timeout.saturating_sub(start.elapsed()))
            .map_err(|_| eyre!("Timed out reading command stdout"))??,
        stderr: err
            .recv_timeout(timeout.saturating_sub(start.elapsed()))
            .map_err(|_| eyre!("Timed out reading command stderr"))??,
    };
    if !output.status.success() {
        return Err(eyre!(
            "Command failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if let Some(writer) = writer {
        writer
            .recv_timeout(timeout.saturating_sub(start.elapsed()))
            .map_err(|_| eyre!("Timed out writing mail"))??;
    }
    Ok(output)
}

fn snapshot(parent: &str) -> Result<(Vec<String>, Records)> {
    // Query all states: --jobs can return an error once the job is purged.
    let mut queue = Command::new("squeue");
    // Inherited squeue preferences must not hide unfinished tasks.
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("SQUEUE_") {
            queue.env_remove(name);
        }
    }
    queue.args(["--array", "--states=all", "--noheader", "--format=%i|%T"]);
    let output = capture(queue, None)?;
    let (known, unfinished) = queue_records(&String::from_utf8(output.stdout)?, parent)?;
    if unfinished {
        return Ok((known, Records::new()));
    }

    let mut accounting = Command::new("sacct");
    accounting.args([
        "--array",
        "--allocations",
        "--parsable2",
        "--noheader",
        "--jobs",
        parent,
        "--starttime=1970-01-01",
        "--format=JobID%64,State%40,ExitCode",
    ]);
    let output = capture(accounting, None)?;
    let records = accounting_records(&String::from_utf8(output.stdout)?, parent)?;
    // A finished task can still be in squeue while its accounting record is delayed.
    let missing = known
        .into_iter()
        .filter(|id| !records.contains_key(id))
        .collect();
    Ok((missing, records))
}

fn queue_records(text: &str, parent: &str) -> Result<(Vec<String>, bool)> {
    let mut known = Vec::new();
    let mut unfinished = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (id, state) = line
            .split_once('|')
            .ok_or_else(|| eyre!("Unexpected squeue output"))?;
        let id = id.trim();
        if parent_id(id).ok().as_deref() == Some(parent) {
            known.push(id.to_string());
            unfinished |= !terminal(state.trim());
        }
    }
    Ok((known, unfinished))
}

fn summary(parent: &str, records: &Records) -> (String, String) {
    let mut counts = BTreeMap::<&str, usize>::new();
    for row in records.values() {
        *counts.entry(&row.state).or_default() += 1;
    }
    let successful = records
        .values()
        .all(|row| row.state == "COMPLETED" && row.exit_code == "0:0");
    let outcome = if successful {
        "completed successfully"
    } else {
        "finished with issues"
    };
    let subject = format!("[Slurmer] Job / array {parent} {outcome}");
    let mut body = format!(
        "Job / array {parent} has finished.\n\nAccounting task records: {}\n",
        records.len()
    );
    for (state, count) in counts {
        body.push_str(&format!("{state}: {count}\n"));
    }
    body.push_str("\nTasks needing attention (up to 50):\n");
    let issues: Vec<_> = records
        .iter()
        .filter(|(_, row)| row.state != "COMPLETED" || row.exit_code != "0:0")
        .collect();
    if issues.is_empty() {
        body.push_str("None. All recorded tasks completed with exit code 0:0.\n");
    }
    for (id, row) in issues.into_iter().take(50) {
        body.push_str(&format!("{id}: {} (exit {})\n", row.state, row.exit_code));
    }
    body.push_str("\nSent by Slurmer after two stable completion checks.\n");
    (subject, body)
}

/// Fall back only when sendmail is absent, never after an ambiguous delivery error.
fn send_mail(email: &str, subject: &str, body: &str) -> Result<()> {
    deliver_mail(email, subject, body, |command, input| {
        capture(command, Some(input)).map(|_| ())
    })
}

fn deliver_mail(
    email: &str,
    subject: &str,
    body: &str,
    mut execute: impl FnMut(Command, Vec<u8>) -> Result<()>,
) -> Result<()> {
    validate_email(email)?;
    let message = format!("To: {email}\nSubject: {subject}\nMIME-Version: 1.0\nContent-Type: text/plain; charset=UTF-8\n\n{body}");
    for program in ["sendmail", "/usr/sbin/sendmail", "/usr/lib/sendmail"] {
        let mut command = Command::new(program);
        command.args(["-t", "-oi"]);
        match execute(command, message.as_bytes().to_vec()) {
            Ok(()) => return Ok(()),
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                continue
            }
            Err(error) => return Err(error),
        }
    }
    let mut command = Command::new("mail");
    command.args(["-s", subject, email]);
    execute(command, body.as_bytes().to_vec())
}

/// Runs until one message is accepted by the local mailer or delivery fails.
/// SLURM errors are reported and retried; mail errors stop to avoid duplicate mail.
pub fn watch(parent: &str, email: &str, report: impl FnMut(String)) -> Result<()> {
    let parent = parent_id(parent)?;
    validate_email(email)?;
    run_watch(
        &parent,
        email,
        POLL_INTERVAL,
        || snapshot(&parent),
        send_mail,
        report,
    )
}

fn run_watch(
    parent: &str,
    email: &str,
    interval: Duration,
    mut read_snapshot: impl FnMut() -> Result<(Vec<String>, Records)>,
    mut deliver: impl FnMut(&str, &str, &str) -> Result<()>,
    mut report: impl FnMut(String),
) -> Result<()> {
    let mut tracker = CompletionTracker::default();
    let mut last_status = String::new();
    report(format!("Watching job / array {parent}; email to {email}"));
    loop {
        let status = match read_snapshot() {
            Ok((active, records)) => {
                let status = if !active.is_empty() {
                    format!(
                        "Job {parent}: waiting for {} observed tasks / accounting records",
                        active.len()
                    )
                } else if records.is_empty() {
                    format!("Job {parent}: no accounting records yet; check job ID and accounting access")
                } else {
                    format!(
                        "Job {parent}: checking {} accounting records for stable completion",
                        records.len()
                    )
                };
                if let Some(records) = tracker.observe(&active, records) {
                    let (subject, body) = summary(parent, &records);
                    deliver(email, &subject, &body)?;
                    report(format!(
                        "Job {parent}: notification accepted by local mailer for {email}"
                    ));
                    return Ok(());
                }
                status
            }
            Err(error) => {
                tracker.candidate = None;
                format!("Job {parent}: {error}; retrying in {}s", interval.as_secs())
            }
        };
        if status != last_status {
            report(status.clone());
            last_status = status;
        }
        thread::sleep(interval);
    }
}

pub struct Notifications {
    watched: BTreeMap<String, (String, bool)>,
    sender: Sender<(String, String, bool)>,
    receiver: Receiver<(String, String, bool)>,
}

impl Notifications {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            watched: BTreeMap::new(),
            sender,
            receiver,
        }
    }

    pub fn start(&mut self, id: &str) -> Result<String> {
        let parent = parent_id(id)?;
        if self.watched.contains_key(&parent) {
            return Ok(format!(
                "Job {parent} already armed or notified in this session"
            ));
        }
        let email = recipient()?;
        self.watched.insert(
            parent.clone(),
            (format!("Waiting; recipient {email}"), false),
        );
        let sender = self.sender.clone();
        let job = parent.clone();
        thread::spawn(move || {
            let result = watch(&job, &email, |message| {
                let _ = sender.send((job.clone(), message, false));
            });
            let message = match result {
                Ok(()) => format!("Job {job}: email accepted by local mailer for {email}"),
                Err(error) => {
                    format!("Job {job}: mail failed: {error}. Check mailer; use --watch to retry.")
                }
            };
            let _ = sender.send((job, message, true));
        });
        Ok(format!(
            "Email armed for entire job / array {parent}. Keep Slurmer open, or use --watch."
        ))
    }

    pub fn messages(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        for (id, message, finished) in self.receiver.try_iter() {
            self.watched.insert(id, (message.clone(), finished));
            messages.push(message);
        }
        messages
    }

    pub fn status_lines(&self) -> Vec<String> {
        self.watched
            .iter()
            .map(|(id, (message, finished))| {
                format!(
                    "{} {id}: {message}",
                    if *finished { "[stopped]" } else { "[watching]" }
                )
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.watched
            .values()
            .filter(|(_, finished)| !finished)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(text: &str) -> Records {
        accounting_records(text, "123").unwrap()
    }

    #[test]
    fn array_rows_share_one_parent() {
        for id in ["123", "123_2", "123_[2-50%4]"] {
            assert_eq!(parent_id(id).unwrap(), "123");
        }
        for id in ["", "--help", "123.batch", "123+1", "123_", "123_;cmd"] {
            assert!(parent_id(id).is_err());
        }
    }

    #[test]
    fn waits_for_all_tasks_and_stable_accounting() {
        let mut tracker = CompletionTracker::default();
        let done = records("123_1|COMPLETED|0:0\n123_2|FAILED|1:0\n");
        assert!(tracker.observe(&["123_2".into()], done.clone()).is_none());
        assert!(tracker
            .observe(&[], records("123_1|COMPLETED|0:0\n"))
            .is_none());
        assert!(tracker.observe(&[], done.clone()).is_none());
        assert_eq!(tracker.observe(&[], done.clone()), Some(done));
    }

    #[test]
    fn missing_accounting_and_nonterminal_states_never_notify() {
        let mut tracker = CompletionTracker::default();
        for state in [
            "RUNNING",
            "PENDING",
            "COMPLETING",
            "REQUEUED",
            "UNKNOWN",
            "CANCELLED+",
        ] {
            let rows = records(&format!("123_1|{state}|0:0\n"));
            assert!(tracker.observe(&[], rows.clone()).is_none());
            assert!(tracker.observe(&[], rows).is_none());
        }
        assert!(tracker.observe(&[], Records::new()).is_none());
    }

    #[test]
    fn ignores_steps_and_unrelated_jobs_and_reports_failures() {
        let rows = records("123_1|COMPLETED|0:0\n123_2|CANCELLED by 100|0:15\n123_2.batch|FAILED|1:0\n999|RUNNING|0:0\n");
        assert_eq!(rows.len(), 2);
        let (subject, body) = summary("123", &rows);
        assert!(subject.contains("issues"));
        assert!(body.contains("CANCELLED: 1"));
        assert!(body.contains("123_2: CANCELLED (exit 0:15)"));
        assert!(accounting_records("123_1|FAILED", "123").is_err());
    }

    #[test]
    fn rejects_email_header_and_option_injection() {
        assert!(validate_email(DEFAULT_EMAIL).is_ok());
        for email in ["-x@example.com", "a@b\nBcc:x@y", "a b@c", "a@@b", "@b"] {
            assert!(validate_email(email).is_err());
        }
    }
    #[test]
    fn queue_checks_all_tasks_and_rejects_malformed_output() {
        let (ids, active) =
            queue_records("123_1|COMPLETED\n123_2|COMPLETING\n999|RUNNING\n", "123").unwrap();
        assert_eq!(ids, ["123_1", "123_2"]);
        assert!(active);
        assert!(
            !queue_records("123_1|CANCELLED\n123_2|PREEMPTED\n", "123")
                .unwrap()
                .1
        );
        assert!(queue_records("unexpected output", "123").is_err());
    }

    #[test]
    fn watcher_retries_slurm_errors_and_delivers_one_array_summary() {
        let complete = records("123_1|COMPLETED|0:0\n123_2|OUT_OF_MEMORY|0:9\n");
        let mut snapshots = std::collections::VecDeque::from([
            Ok((vec!["123_1".into(), "123_2".into()], Records::new())),
            Ok((vec![], complete.clone())),
            Err(eyre!("accounting unavailable")),
            Ok((vec![], complete.clone())),
            Ok((vec![], complete)),
        ]);
        let mut sent = Vec::new();
        run_watch(
            "123",
            DEFAULT_EMAIL,
            Duration::ZERO,
            || snapshots.pop_front().expect("unexpected extra polling"),
            |to, subject, body| {
                sent.push((to.to_string(), subject.to_string(), body.to_string()));
                Ok(())
            },
            |_| {},
        )
        .unwrap();
        assert!(snapshots.is_empty());
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, DEFAULT_EMAIL);
        assert!(sent[0].1.contains("issues"));
        assert!(sent[0].2.contains("OUT_OF_MEMORY: 1"));
    }

    #[test]
    fn ambiguous_mail_failure_is_not_retried() {
        let complete = records("123|COMPLETED|0:0\n");
        let mut attempts = 0;
        let result = run_watch(
            "123",
            DEFAULT_EMAIL,
            Duration::ZERO,
            || Ok((vec![], complete.clone())),
            |_, _, _| {
                attempts += 1;
                Err(eyre!("mailer timed out"))
            },
            |_| {},
        );
        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[test]
    fn status_retains_finished_watches_and_deduplicates_parent_ids() {
        let mut notifications = Notifications::new();
        notifications
            .watched
            .insert("123".into(), ("armed".into(), false));
        assert_eq!(notifications.count(), 1);
        assert!(notifications.start("123_2").unwrap().contains("already"));
        notifications
            .sender
            .send(("123".into(), "accepted".into(), true))
            .unwrap();
        assert_eq!(notifications.messages(), ["accepted"]);
        assert_eq!(notifications.count(), 0);
        assert!(notifications.status_lines()[0].contains("accepted"));
    }

    #[test]
    #[cfg(unix)]
    fn command_transport_captures_stdin_errors_and_timeouts_without_sending_mail() {
        let mut echo = Command::new("/bin/sh");
        echo.args(["-c", "cat"]);
        let output = capture(echo, Some(b"test mail body".to_vec())).unwrap();
        assert_eq!(output.stdout, b"test mail body");
        let mut failing = Command::new("/bin/sh");
        failing.args(["-c", "echo relay-unavailable >&2; exit 1"]);
        assert!(capture(failing, None)
            .unwrap_err()
            .to_string()
            .contains("relay-unavailable"));
        let mut sleeping = Command::new("/bin/sleep");
        sleeping.arg("5");
        assert!(
            capture_with_timeout(sleeping, None, Duration::from_millis(20))
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
    }
    #[test]
    fn mail_transport_uses_headers_and_falls_back_only_if_missing() {
        let mut calls = 0;
        deliver_mail(
            DEFAULT_EMAIL,
            "Job 123 finished",
            "All done",
            |cmd, body| {
                calls += 1;
                assert_eq!(cmd.get_program(), "sendmail");
                assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["-t", "-oi"]);
                let text = String::from_utf8(body).unwrap();
                assert!(
                    text.starts_with(&format!("To: {DEFAULT_EMAIL}\nSubject: Job 123 finished\n"))
                );
                assert!(text.ends_with("\n\nAll done"));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
        calls = 0;
        deliver_mail(
            DEFAULT_EMAIL,
            "Job 123 finished",
            "All done",
            |cmd, body| {
                calls += 1;
                if cmd.get_program() != "mail" {
                    return Err(io::Error::from(io::ErrorKind::NotFound).into());
                }
                assert_eq!(
                    cmd.get_args().collect::<Vec<_>>(),
                    ["-s", "Job 123 finished", DEFAULT_EMAIL]
                );
                assert_eq!(body, b"All done");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls, 4);
        calls = 0;
        assert!(
            deliver_mail(DEFAULT_EMAIL, "Job 123 finished", "All done", |_, _| {
                calls += 1;
                Err(eyre!("relay rejected the message"))
            })
            .is_err()
        );
        assert_eq!(calls, 1);
    }
}
