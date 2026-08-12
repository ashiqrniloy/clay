//! Bounded, Clay-owned fuzzy subsequence scoring for transient menus.
//!
//! Matching is case-insensitive and operates on inert text only. Inputs are
//! capped before Unicode case expansion so a malformed or unexpectedly long
//! label cannot turn menu filtering into unbounded work.

const MAX_INPUT_CHARS: usize = 256;
const MATCH_SCORE: i32 = 10;
const WORD_BOUNDARY_BONUS: i32 = 8;
const CONSECUTIVE_BONUS: i32 = 8;
const GAP_PENALTY: i32 = 1;
const EARLY_POSITION_BONUS: i32 = 4;

/// Scores a case-insensitive subsequence match.
///
/// Returns `None` when every query character cannot be found in order. Higher
/// scores favor word boundaries, consecutive matches, earlier positions, and
/// denser matches. Empty queries score zero.
pub(crate) fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let query = normalize(query);
    let candidate = normalize(candidate);
    score_normalized(&query, &candidate)
}

/// Returns the best fuzzy score across bounded searchable fields.
pub(crate) fn fuzzy_score_fields<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<i32> {
    let query = normalize(query);
    candidates
        .into_iter()
        .filter_map(|candidate| score_normalized(&query, &normalize(candidate)))
        .max()
}

fn normalize(input: &str) -> Vec<char> {
    input
        .chars()
        .take(MAX_INPUT_CHARS)
        .flat_map(char::to_lowercase)
        .collect()
}

fn score_normalized(query: &[char], candidate: &[char]) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    if query.len() > candidate.len() {
        return None;
    }

    let mut previous = vec![None; candidate.len()];
    for (query_index, query_char) in query.iter().enumerate() {
        let mut current = vec![None; candidate.len()];
        let mut best_gap: Option<i32> = None;

        for (candidate_index, candidate_char) in candidate.iter().enumerate() {
            if candidate_index >= 2
                && let Some(previous_score) = previous[candidate_index - 2]
            {
                let gap_score = previous_score + GAP_PENALTY * (candidate_index as i32 - 2);
                best_gap = Some(best_gap.map_or(gap_score, |best| best.max(gap_score)));
            }

            if query_char != candidate_char {
                continue;
            }

            let character_score = match_score(candidate, candidate_index);
            current[candidate_index] = if query_index == 0 {
                Some(character_score)
            } else {
                let adjacent = candidate_index
                    .checked_sub(1)
                    .and_then(|index| previous[index])
                    .map(|score| score + CONSECUTIVE_BONUS + character_score);
                let separated = best_gap.map(|score| {
                    score - GAP_PENALTY * (candidate_index as i32 - 1) + character_score
                });
                adjacent.into_iter().chain(separated).max()
            };
        }

        previous = current;
    }

    previous.into_iter().flatten().max()
}

fn match_score(candidate: &[char], index: usize) -> i32 {
    let boundary_bonus = if is_word_character(candidate[index])
        && (index == 0 || !is_word_character(candidate[index - 1]))
    {
        WORD_BOUNDARY_BONUS
    } else {
        0
    };
    MATCH_SCORE + boundary_bonus + EARLY_POSITION_BONUS.saturating_sub(index as i32)
}

fn is_word_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_matches_when_substring_does_not() {
        assert!(fuzzy_score("ccop", "Control Center Open").is_some());
    }

    #[test]
    fn word_boundary_match_outranks_interior_match() {
        assert!(fuzzy_score("c", "Control") > fuzzy_score("c", "sc"));
    }

    #[test]
    fn consecutive_match_outranks_same_start_with_gap() {
        assert!(fuzzy_score("ab", "ab") > fuzzy_score("ab", "a-b"));
    }

    #[test]
    fn non_subsequence_returns_none() {
        assert_eq!(fuzzy_score("xz", "example"), None);
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(fuzzy_score("CoP", "Control Open").is_some());
    }

    #[test]
    fn unicode_case_mapping_stays_panic_free() {
        assert!(fuzzy_score("Ä", "ä").is_some());
    }

    #[test]
    fn empty_query_has_zero_score() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn long_inputs_are_bounded() {
        let query = "x".repeat(MAX_INPUT_CHARS + 1);
        assert_eq!(fuzzy_score(&query, "x"), None);
    }
}
