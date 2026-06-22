use crate::slurm::Job;

pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i64> {
    let query = query.to_lowercase();
    if query.is_empty() {
        return Some(0);
    }

    let candidate = candidate.to_lowercase();
    let mut chars = candidate.char_indices();
    let mut score = 0_i64;
    let mut previous = None;

    for needle in query.chars() {
        let (index, _) = chars.find(|(_, hay)| *hay == needle)?;
        score += 10;
        if let Some(previous) = previous {
            let gap = index.saturating_sub(previous);
            score -= gap.min(9) as i64;
            if gap == needle.len_utf8() {
                score += 8;
            }
        } else {
            score -= index.min(12) as i64;
        }
        previous = Some(index);
    }

    Some(score)
}

pub fn job_score(query: &str, job: &Job) -> Option<i64> {
    let fields = [
        job.id.as_str(),
        job.name.as_str(),
        job.user.as_str(),
        job.partition.as_str(),
        job.qos.as_str(),
        job.node.as_deref().unwrap_or_default(),
    ];

    fields
        .iter()
        .filter_map(|field| fuzzy_score(query, field))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn consecutive_matches_rank_higher() {
        assert!(fuzzy_score("gpu", "gpu-worker") > fuzzy_score("gpu", "great-purple-unit"));
    }

    #[test]
    fn search_is_case_insensitive_and_unicode_safe() {
        assert!(fuzzy_score("紫", "紫色-job").is_some());
        assert!(fuzzy_score("GPU", "gpu-worker").is_some());
    }
}
