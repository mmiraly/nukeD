use crate::display::{age_label, bytes};
use crate::scanner::DependencyFolder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchScore(pub i64);

pub fn score(query: &str, candidate: &str) -> Option<MatchScore> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Some(MatchScore(0));
    }

    let candidate = candidate.to_ascii_lowercase();
    if candidate.contains(&query) {
        return Some(MatchScore(10_000 - candidate.len() as i64));
    }

    let mut score = 0i64;
    let mut last_match = None;
    let mut chars = candidate.char_indices();

    for needle in query.chars() {
        let mut found = None;
        for (idx, hay) in chars.by_ref() {
            if hay == needle {
                found = Some(idx);
                break;
            }
        }

        let idx = found?;
        score += 100;
        if let Some(prev) = last_match {
            let gap = idx.saturating_sub(prev + 1) as i64;
            score -= gap.min(25);
        } else {
            score -= (idx as i64).min(40);
        }
        last_match = Some(idx);
    }

    Some(MatchScore(score))
}

pub fn folder_haystack(folder: &DependencyFolder) -> String {
    format!(
        "{} {} {} {} {}",
        folder.kind.label(),
        folder.project_path.display(),
        folder.path.display(),
        bytes(folder.size_bytes),
        age_label(folder.age)
    )
}

pub fn matching_indices(folders: &[DependencyFolder], query: &str) -> Vec<usize> {
    let query = query.trim();
    let mut matches: Vec<(usize, MatchScore)> = folders
        .iter()
        .enumerate()
        .filter_map(|(idx, folder)| {
            score(query, &folder_haystack(folder)).map(|score| (idx, score))
        })
        .collect();

    if !query.is_empty() {
        matches.sort_by(|(left_idx, left_score), (right_idx, right_score)| {
            right_score
                .0
                .cmp(&left_score.0)
                .then(left_idx.cmp(right_idx))
        });
    }

    matches.into_iter().map(|(idx, _)| idx).collect()
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn matches_substrings() {
        assert!(score("node", "/tmp/app/node_modules").is_some());
    }

    #[test]
    fn matches_ordered_fuzzy_text() {
        assert!(score("nmod", "/tmp/app/node_modules").is_some());
    }

    #[test]
    fn rejects_unordered_text() {
        assert!(score("zyx", "/tmp/app/node_modules").is_none());
    }

    #[test]
    fn ranks_substrings_above_sparse_matches() {
        let exact = score("dep", "/tmp/deps").unwrap();
        let sparse = score("dep", "/tmp/different-example-path").unwrap();
        assert!(exact.0 > sparse.0);
    }
}
