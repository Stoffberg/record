mod opencode;

use chrono::{DateTime, Utc};

pub use opencode::OpencodeProvider;

pub struct AgentWorkSlice {
    pub agent: String,
    pub project: String,
    pub session_ref: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_secs: i64,
}

pub trait AgentProvider: Send {
    fn name(&self) -> &str;
    fn scan(&self, since_ms: i64) -> Vec<AgentWorkSlice>;
}

pub fn merge_intervals(mut intervals: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by_key(|i| i.0);
    let mut merged: Vec<(i64, i64)> = vec![intervals[0]];
    for interval in intervals.into_iter().skip(1) {
        let last = merged.last_mut().unwrap();
        if interval.0 <= last.1 {
            last.1 = last.1.max(interval.1);
        } else {
            merged.push(interval);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_non_overlapping() {
        let intervals = vec![(1, 5), (10, 15), (20, 25)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 5), (10, 15), (20, 25)]);
    }

    #[test]
    fn merge_overlapping() {
        let intervals = vec![(1, 10), (5, 15), (12, 20)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 20)]);
    }

    #[test]
    fn merge_adjacent() {
        let intervals = vec![(1, 5), (5, 10)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 10)]);
    }

    #[test]
    fn merge_empty() {
        let intervals: Vec<(i64, i64)> = vec![];
        let result = merge_intervals(intervals);
        assert!(result.is_empty());
    }

    #[test]
    fn merge_single() {
        let intervals = vec![(1, 5)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 5)]);
    }

    #[test]
    fn merge_unsorted() {
        let intervals = vec![(20, 25), (1, 5), (3, 8)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 8), (20, 25)]);
    }

    #[test]
    fn merge_contained() {
        let intervals = vec![(1, 20), (5, 10), (8, 15)];
        let result = merge_intervals(intervals);
        assert_eq!(result, vec![(1, 20)]);
    }
}
