use async_process::{Command, Output};
use color_eyre::eyre::Error;
use color_eyre::Result;

const DEFAULT_FORMAT: &str =
    "JobIDRaw,JobName,User,State,ExitCode,Elapsed,Start,End,Partition,Account,AllocCPUS,ReqMem,MaxRSS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountingStateFilter {
    All,
    Completed,
    Failed,
    Cancelled,
    Timeout,
    NodeFail,
    Preempted,
}

impl AccountingStateFilter {
    pub const ALL: [AccountingStateFilter; 7] = [
        AccountingStateFilter::All,
        AccountingStateFilter::Completed,
        AccountingStateFilter::Failed,
        AccountingStateFilter::Cancelled,
        AccountingStateFilter::Timeout,
        AccountingStateFilter::NodeFail,
        AccountingStateFilter::Preempted,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AccountingStateFilter::All => "all",
            AccountingStateFilter::Completed => "completed",
            AccountingStateFilter::Failed => "failed",
            AccountingStateFilter::Cancelled => "cancelled",
            AccountingStateFilter::Timeout => "timeout",
            AccountingStateFilter::NodeFail => "node_fail",
            AccountingStateFilter::Preempted => "preempted",
        }
    }

    fn states(self) -> Option<&'static str> {
        match self {
            AccountingStateFilter::All => None,
            AccountingStateFilter::Completed => Some("COMPLETED"),
            AccountingStateFilter::Failed => Some("FAILED"),
            AccountingStateFilter::Cancelled => Some("CANCELLED"),
            AccountingStateFilter::Timeout => Some("TIMEOUT"),
            AccountingStateFilter::NodeFail => Some("NODE_FAIL"),
            AccountingStateFilter::Preempted => Some("PREEMPTED"),
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacctOptions {
    pub user: Option<String>,
    pub days: u16,
    pub state_filter: AccountingStateFilter,
    pub format: String,
}

impl Default for SacctOptions {
    fn default() -> Self {
        Self {
            user: std::env::var("USER").ok().filter(|user| !user.is_empty()),
            days: 7,
            state_filter: AccountingStateFilter::All,
            format: DEFAULT_FORMAT.to_string(),
        }
    }
}

impl SacctOptions {
    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec![
            "--parsable2".to_string(),
            "--noheader".to_string(),
            "--format".to_string(),
            self.format.clone(),
            "--starttime".to_string(),
            format!("now-{}days", self.days.max(1)),
        ];

        if let Some(user) = &self.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        if let Some(states) = self.state_filter.states() {
            args.push("--state".to_string());
            args.push(states.to_string());
        }

        args
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountingJob {
    pub id: String,
    pub name: String,
    pub user: String,
    pub state: String,
    pub exit_code: String,
    pub elapsed: String,
    pub start: String,
    pub end: String,
    pub partition: String,
    pub account: String,
    pub cpus: Option<u32>,
    pub requested_memory: Option<String>,
    pub max_rss: Option<String>,
}

pub async fn run_sacct(options: &SacctOptions) -> Result<Vec<AccountingJob>> {
    let args = options.to_args();
    let output = match Command::new("sacct").args(&args).output().await {
        Ok(output) => output,
        Err(error) => return Err(Error::new(error)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(color_eyre::eyre::eyre!(
            "sacct failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }

    Ok(parse_sacct_output(&output))
}

fn parse_sacct_output(output: &Output) -> Vec<AccountingJob> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_sacct_text(&stdout)
}

fn parse_sacct_text(stdout: &str) -> Vec<AccountingJob> {
    stdout
        .lines()
        .filter_map(|line| {
            let parts = line.split('|').collect::<Vec<_>>();
            if parts.len() < 13 {
                return None;
            }

            Some(AccountingJob {
                id: clean(parts[0]).to_string(),
                name: clean(parts[1]).to_string(),
                user: clean(parts[2]).to_string(),
                state: clean(parts[3]).to_string(),
                exit_code: clean(parts[4]).to_string(),
                elapsed: clean(parts[5]).to_string(),
                start: clean(parts[6]).to_string(),
                end: clean(parts[7]).to_string(),
                partition: clean(parts[8]).to_string(),
                account: clean(parts[9]).to_string(),
                cpus: clean(parts[10]).parse::<u32>().ok(),
                requested_memory: optional(parts[11]),
                max_rss: optional(parts[12]),
            })
        })
        .collect()
}

fn clean(value: &str) -> &str {
    let value = value.trim();
    if value == "Unknown" || value == "N/A" {
        ""
    } else {
        value
    }
}

fn optional(value: &str) -> Option<String> {
    let value = clean(value);
    (!value.is_empty() && value != "-").then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sacct_args_include_user_range_format_and_state() {
        let options = SacctOptions {
            user: Some("shelley".to_string()),
            days: 30,
            state_filter: AccountingStateFilter::Failed,
            format: DEFAULT_FORMAT.to_string(),
        };
        let args = options.to_args();
        assert!(args.contains(&"--parsable2".to_string()));
        assert!(args.contains(&"--noheader".to_string()));
        assert_eq!(
            args[args.iter().position(|arg| arg == "--user").unwrap() + 1],
            "shelley"
        );
        assert_eq!(
            args[args.iter().position(|arg| arg == "--starttime").unwrap() + 1],
            "now-30days"
        );
        assert_eq!(
            args[args.iter().position(|arg| arg == "--state").unwrap() + 1],
            "FAILED"
        );
    }

    #[test]
    fn all_state_filter_omits_state_argument() {
        let args = SacctOptions::default().to_args();
        assert!(!args.iter().any(|arg| arg == "--state"));
    }

    #[test]
    fn state_filters_cycle_and_map_to_sacct_states() {
        let expected = [
            (AccountingStateFilter::All, None),
            (AccountingStateFilter::Completed, Some("COMPLETED")),
            (AccountingStateFilter::Failed, Some("FAILED")),
            (AccountingStateFilter::Cancelled, Some("CANCELLED")),
            (AccountingStateFilter::Timeout, Some("TIMEOUT")),
            (AccountingStateFilter::NodeFail, Some("NODE_FAIL")),
            (AccountingStateFilter::Preempted, Some("PREEMPTED")),
        ];

        for (filter, state) in expected {
            assert_eq!(filter.states(), state);
        }

        let mut filter = AccountingStateFilter::All;
        for expected_filter in AccountingStateFilter::ALL.into_iter().skip(1) {
            filter = filter.next();
            assert_eq!(filter, expected_filter);
        }
        assert_eq!(filter.next(), AccountingStateFilter::All);
    }

    #[test]
    fn parser_reads_jobs_and_optional_memory_fields() {
        let jobs = parse_sacct_text(
            "42|train|shelley|FAILED|1:0|00:03:12|2026-06-28T10:00:00|2026-06-28T10:03:12|gpu|lab|8|32G|1200K\n43|ok|shelley|COMPLETED|0:0|00:01:00|Unknown|Unknown|cpu|lab|1|-|\n",
        );
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, "42");
        assert_eq!(jobs[0].max_rss.as_deref(), Some("1200K"));
        assert_eq!(jobs[1].start, "");
        assert_eq!(jobs[1].requested_memory, None);
        assert_eq!(jobs[1].max_rss, None);
    }

    #[test]
    fn parser_skips_malformed_rows_and_keeps_array_ids() {
        let jobs = parse_sacct_text(
            "malformed|row\n123_4|array|shelley|CANCELLED|0:15|00:00:05|start|end|gpu|lab|2|8G|-\n123.batch|batch|shelley|FAILED|1:0|00:00:01|start|end|gpu|lab|2|8G|2M\n",
        );
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, "123_4");
        assert_eq!(jobs[1].id, "123.batch");
    }
}
