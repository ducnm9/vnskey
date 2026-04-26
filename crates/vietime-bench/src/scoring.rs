// SPDX-License-Identifier: GPL-3.0-or-later
//
// Scoring engine (BEN-13).
//
// Compares captured text against expected output and produces per-vector +
// per-combo scores. Two metrics:
//   - exact match (boolean)
//   - normalised edit distance via Levenshtein
//
// Spec ref: `spec/03-phase3-test-suite.md` §B.7.

use serde::{Deserialize, Serialize};

/// Score for a single test vector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorScore {
    pub vector_id: String,
    pub expected: String,
    pub actual: String,
    pub exact_match: bool,
    pub edit_distance: usize,
    pub normalised_distance: f64,
}

/// Aggregate score for one (engine × app × session × mode) combo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComboScore {
    pub vectors_tested: u32,
    pub exact_match_count: u32,
    pub accuracy_pct: f64,
    pub edit_distance_total: u32,
    pub weighted_score: f64,
}

/// Score a single vector by comparing `actual` (captured from the app) against
/// `expected` (from the test vector file).
#[must_use]
pub fn score_vector(vector_id: &str, expected: &str, actual: &str) -> VectorScore {
    let exact_match = expected == actual;
    let edit_distance = strsim::levenshtein(expected, actual);
    let max_len = expected.len().max(actual.len());
    let normalised_distance = if max_len == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        let d = edit_distance as f64 / max_len as f64;
        d
    };

    VectorScore {
        vector_id: vector_id.to_owned(),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
        exact_match,
        edit_distance,
        normalised_distance,
    }
}

/// Aggregate a slice of per-vector scores into a combo-level summary.
#[must_use]
pub fn aggregate_scores(scores: &[VectorScore]) -> ComboScore {
    if scores.is_empty() {
        return ComboScore {
            vectors_tested: 0,
            exact_match_count: 0,
            accuracy_pct: 0.0,
            edit_distance_total: 0,
            weighted_score: 0.0,
        };
    }

    #[allow(clippy::cast_possible_truncation)]
    let vectors_tested = scores.len() as u32;
    #[allow(clippy::cast_possible_truncation)]
    let exact_match_count = scores.iter().filter(|s| s.exact_match).count() as u32;

    #[allow(clippy::cast_precision_loss)]
    let accuracy_pct = f64::from(exact_match_count) / f64::from(vectors_tested) * 100.0;

    #[allow(clippy::cast_possible_truncation)]
    let edit_distance_total = scores.iter().map(|s| s.edit_distance).sum::<usize>() as u32;

    let weighted_score =
        1.0 - scores.iter().map(|s| s.normalised_distance).sum::<f64>() / f64::from(vectors_tested);

    ComboScore {
        vectors_tested,
        exact_match_count,
        accuracy_pct,
        edit_distance_total,
        weighted_score,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_scores_perfectly() {
        let s = score_vector("T001", "tiếng Việt", "tiếng Việt");
        assert!(s.exact_match);
        assert_eq!(s.edit_distance, 0);
        assert!((s.normalised_distance).abs() < f64::EPSILON);
    }

    #[test]
    fn mismatch_computes_edit_distance() {
        let s = score_vector("T002", "người", "nnggười");
        assert!(!s.exact_match);
        assert!(s.edit_distance > 0);
        assert!(s.normalised_distance > 0.0);
        assert!(s.normalised_distance <= 1.0);
    }

    #[test]
    fn empty_strings_score_zero_distance() {
        let s = score_vector("T003", "", "");
        assert!(s.exact_match);
        assert_eq!(s.edit_distance, 0);
        assert!((s.normalised_distance).abs() < f64::EPSILON);
    }

    #[test]
    fn completely_wrong_scores_near_one() {
        let s = score_vector("T004", "abc", "xyz");
        assert!(!s.exact_match);
        assert_eq!(s.edit_distance, 3);
        assert!((s.normalised_distance - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregate_empty_list() {
        let agg = aggregate_scores(&[]);
        assert_eq!(agg.vectors_tested, 0);
        assert!((agg.accuracy_pct).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregate_all_perfect() {
        let scores = vec![
            score_vector("T001", "â", "â"),
            score_vector("T002", "ê", "ê"),
            score_vector("T003", "ô", "ô"),
        ];
        let agg = aggregate_scores(&scores);
        assert_eq!(agg.vectors_tested, 3);
        assert_eq!(agg.exact_match_count, 3);
        assert!((agg.accuracy_pct - 100.0).abs() < f64::EPSILON);
        assert_eq!(agg.edit_distance_total, 0);
        assert!((agg.weighted_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregate_mixed_results() {
        let scores = vec![score_vector("T001", "â", "â"), score_vector("T002", "người", "ngưới")];
        let agg = aggregate_scores(&scores);
        assert_eq!(agg.vectors_tested, 2);
        assert_eq!(agg.exact_match_count, 1);
        assert!((agg.accuracy_pct - 50.0).abs() < f64::EPSILON);
        assert!(agg.edit_distance_total > 0);
        assert!(agg.weighted_score > 0.0);
        assert!(agg.weighted_score < 1.0);
    }

    #[test]
    fn levenshtein_handles_unicode_graphemes() {
        let s = score_vector("T005", "xin chào", "xin chao");
        assert!(!s.exact_match);
        assert!(s.edit_distance > 0);
    }

    #[test]
    fn serde_round_trip_vector_score() {
        let s = score_vector("T001", "â", "â");
        let json = serde_json::to_string(&s).unwrap();
        let back: VectorScore = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn serde_round_trip_combo_score() {
        let scores = vec![score_vector("T001", "â", "â")];
        let agg = aggregate_scores(&scores);
        let json = serde_json::to_string(&agg).unwrap();
        let back: ComboScore = serde_json::from_str(&json).unwrap();
        assert_eq!(agg, back);
    }
}
