//! Simple fuzzy string matching.

/// Score a candidate string against a query using fuzzy matching.
///
/// Returns a score > 0 if the candidate matches, 0 otherwise.
/// Higher scores indicate better matches.
pub fn fuzzy_score(query: &str, candidate: &str) -> u32 {
    if query.is_empty() {
        return 1; // empty query matches everything
    }

    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let cand_chars: Vec<char> = candidate.chars().collect();
    let cand_lower: Vec<char> = candidate.to_lowercase().chars().collect();

    let mut qi = 0;
    let mut score: u32 = 0;
    let mut prev_match = false;
    let mut prev_was_separator = true;

    for (ci, &ch) in cand_lower.iter().enumerate() {
        let is_boundary = prev_was_separator
            || (ci > 0 && cand_chars[ci].is_uppercase() && cand_chars[ci - 1].is_lowercase());

        if qi < query_lower.len() && ch == query_lower[qi] {
            score += 1;

            // Bonus for consecutive matches
            if prev_match {
                score += 2;
            }

            // Bonus for matching at word boundaries
            if is_boundary {
                score += 3;
            }

            // Bonus for matching at the start
            if ci == 0 {
                score += 5;
            }

            qi += 1;
            prev_match = true;
        } else {
            prev_match = false;
        }

        prev_was_separator = matches!(ch, '_' | '-' | ' ' | '/' | '.');
    }

    if qi == query_lower.len() {
        score
    } else {
        0 // Not all query chars matched
    }
}

/// Filter and rank candidates by fuzzy match against a query.
pub fn fuzzy_filter<'a>(query: &str, candidates: &'a [&str]) -> Vec<(&'a str, u32)> {
    let mut results: Vec<(&str, u32)> = candidates
        .iter()
        .filter_map(|&c| {
            let s = fuzzy_score(query, c);
            if s > 0 { Some((c, s)) } else { None }
        })
        .collect();
    results.sort_by_key(|b| std::cmp::Reverse(b.1));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_fuzzy() {
        assert!(fuzzy_score("btn", "Button") > 0);
        assert!(fuzzy_score("xyz", "Button") == 0);
        // Word boundary bonus
        assert!(fuzzy_score("fb", "FooBar") > fuzzy_score("fb", "flab"));
    }

    #[test]
    fn fuzzy_filter_ranking() {
        let candidates = &["TextInput", "TextArea", "Button", "Tooltip"];
        let results = fuzzy_filter("tex", candidates);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "TextInput");
    }
}
