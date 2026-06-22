use async_process::{Command, Output};
use color_eyre::eyre::Error;
use color_eyre::Result;
use std::str::FromStr;

use super::Job;
use super::JobState;

#[derive(Debug, Clone)]
pub struct SqueueOptions {
    pub user: Option<String>,
    pub states: Vec<JobState>,
    pub partitions: Vec<String>,
    pub qos: Vec<String>,
    pub name_filter: Option<String>,
    pub node_filter: Option<String>,
    pub format: String,
    pub sorts: Vec<(String, bool)>,
}

impl Default for SqueueOptions {
    fn default() -> Self {
        // Default username from environment
        let username = std::env::var("USER").unwrap_or_else(|_| "".to_string());

        // Default sort options
        let sorts = vec![("i".to_string(), true)];

        Self {
            user: Some(username),
            states: Vec::new(),
            partitions: Vec::new(),
            qos: Vec::new(),
            name_filter: None,
            node_filter: None,
            format: "%i|%j|%u|%T|%M|%N|%C|%m|%P|%q".to_string(), // JobID|Name|User|State|Time|Nodes|CPUs|Memory|Partition|QOS
            sorts,
        }
    }
}

impl SqueueOptions {
    // Get the current format codes as a Vec<&str>
    pub fn format_codes(&self) -> Vec<&str> {
        self.format.split('|').collect()
    }

    // Validate the format string to ensure it contains valid format codes
    pub fn validate_format(&self) -> bool {
        let codes = self.format_codes();
        !codes.is_empty() && codes.iter().all(|code| code.starts_with('%'))
    }
}

impl SqueueOptions {
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // User filter
        if let Some(user) = &self.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        // State filter
        if !self.states.is_empty() {
            let states = self
                .states
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(",");
            args.push("--states".to_string());
            args.push(states);
        }

        // Partition filter
        if !self.partitions.is_empty() {
            let partitions = self.partitions.join(",");
            args.push("--partition".to_string());
            args.push(partitions);
        }

        // QOS filter
        if !self.qos.is_empty() {
            let qos = self.qos.join(",");
            args.push("--qos".to_string());
            args.push(qos);
        }

        // Name filter is now handled internally by the application
        // so we don't pass it to squeue

        // Format specification
        args.push("--format".to_string());
        args.push(self.format.clone());

        // Sort options
        if !self.sorts.is_empty() {
            // Create a sort string from the sorts map
            let sort_string = self
                .sorts
                .iter()
                .map(|(field, ascending)| {
                    let prefix = if *ascending { "" } else { "-" };
                    format!("{}{}", prefix, field)
                })
                .collect::<Vec<_>>()
                .join(",");

            args.push("--sort".to_string());
            args.push(sort_string);
        }

        // No header flag to make parsing easier
        args.push("--noheader".to_string());

        args
    }
}

pub async fn run_squeue(options: &SqueueOptions) -> Result<Vec<Job>> {
    let args = options.to_args();
    // eprintln!("Running squeue with args: {:?}", args);

    // Validate format string
    if !options.validate_format() {
        // eprintln!("Warning: Invalid format string: {}", options.format);
        return Ok(Vec::new());
    }

    let output = match Command::new("squeue").args(&args).output().await {
        Ok(output) => {
            // eprintln!("Running squeue command completed");
            output
        }
        Err(e) => {
            // eprintln!("Error running squeue command: {}", e);
            // return Ok(Vec::new());
            return Err(Error::new(e));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(color_eyre::eyre::eyre!(
            "squeue failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }

    // Pass the format options with the output to ensure correct parsing
    parse_squeue_output(&output, &options.format)
}

/// Dynamic parsing of squeue output based on the provided format string
fn parse_squeue_output(output: &Output, format: &str) -> Result<Vec<Job>> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_squeue_text(&stdout, format))
}

fn parse_squeue_text(stdout: &str, format: &str) -> Vec<Job> {
    let mut jobs = Vec::new();
    let lines = stdout.lines();

    // Note: name_filter is now applied in App::refresh_jobs, not here

    // Handle empty output
    if stdout.trim().is_empty() {
        // eprintln!("No jobs found in squeue output");
        return jobs;
    }

    let format_codes: Vec<&str> = format.split('|').collect();

    if format_codes.is_empty() {
        // eprintln!("Warning: Empty format codes, using default format");
        return jobs;
    }

    // eprintln!("Format codes: {:?}", format_codes);

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < format_codes.len() {
            // eprintln!("Skipping invalid line: {}", line);
            continue;
        }

        let mut job = Job::default();

        // Ensure we have enough parts to match the format codes
        for (i, part) in parts.iter().enumerate() {
            if i >= format_codes.len() {
                break;
            }

            let value = part.trim().to_string();
            // Skip empty values or "N/A"
            if value.is_empty() || value == "N/A" {
                continue;
            }

            match format_codes[i] {
                "%i" | "%A" => job.id = value,
                "%j" => job.name = value,
                "%u" => job.user = value,
                "%T" => {
                    job.state = JobState::from_str(&value).unwrap_or_else(|_| {
                        // eprintln!("Failed to parse job state: {}", value);
                        JobState::Other
                    })
                }
                "%M" => job.time = value,
                "%D" => {
                    job.nodes = value.parse::<u32>().unwrap_or_else(|_| {
                        // eprintln!("Failed to parse node count: {}", value);
                        0
                    })
                }
                "%N" => job.node = Some(value),
                "%C" => {
                    job.cpus = value.parse::<u32>().unwrap_or_else(|_| {
                        // eprintln!("Failed to parse CPU count: {}", value);
                        0
                    })
                }
                "%m" => job.memory = value,
                "%P" => job.partition = value,
                "%q" => job.qos = value,
                "%a" => job.account = Some(value),
                "%Q" => job.priority = value.parse::<u32>().ok(),
                "%Z" => job.work_dir = Some(value),
                "%V" => job.submit_time = Some(value),
                "%S" => job.start_time = Some(value),
                "%e" => job.end_time = Some(value),
                "%R" => job.pending_reason = Some(value),
                _ => {
                    // eprintln!("Unknown format code: {}", format_codes[i]);
                }
            }
        }

        jobs.push(job);
    }

    jobs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_arguments_preserve_priority_order() {
        let options = SqueueOptions {
            sorts: vec![("P".to_string(), false), ("i".to_string(), true)],
            ..SqueueOptions::default()
        };
        let args = options.to_args();
        let sort_index = args.iter().position(|arg| arg == "--sort").unwrap();
        assert_eq!(args[sort_index + 1], "-P,i");
    }

    #[test]
    fn invalid_format_is_rejected() {
        let options = SqueueOptions {
            format: "%i|name".to_string(),
            ..SqueueOptions::default()
        };
        assert!(!options.validate_format());
    }

    #[test]
    fn parser_skips_malformed_rows_and_reads_pending_reason() {
        let jobs = parse_squeue_text("42|train|PENDING|Resources\nmalformed|row\n", "%i|%j|%T|%R");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "42");
        assert_eq!(jobs[0].pending_reason.as_deref(), Some("Resources"));
    }
}
