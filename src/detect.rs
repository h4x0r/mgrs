use regex::Regex;
use std::sync::OnceLock;

fn mgrs_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b\d{1,2}\s*[C-X]\s*[A-Z]{2}\s*\d{2,10}\b").unwrap())
}

/// Check if a string value looks like an MGRS coordinate.
pub fn is_likely_mgrs(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 7 && mgrs_regex().is_match(trimmed)
}

/// Auto-detect which column in a set of CSV records contains MGRS coordinates.
pub fn detect_mgrs_column(records: &[csv::StringRecord]) -> Option<usize> {
    if records.is_empty() {
        return None;
    }

    let num_columns = records[0].len();
    let mut column_scores = vec![0usize; num_columns];

    for record in records.iter().take(100) {
        for (col_idx, field) in record.iter().enumerate() {
            if is_likely_mgrs(field.trim()) {
                column_scores[col_idx] += 1;
            }
        }
    }

    column_scores
        .iter()
        .enumerate()
        .max_by_key(|&(_, score)| score)
        .filter(|&(_, score)| *score > 0)
        .map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_mgrs_valid() {
        assert!(is_likely_mgrs("18SUJ2337006519"));
        assert!(is_likely_mgrs("4QFJ1234567890"));
        assert!(is_likely_mgrs("31U DQ 48251 11932"));
    }

    #[test]
    fn test_is_likely_mgrs_invalid() {
        assert!(!is_likely_mgrs("hello"));
        assert!(!is_likely_mgrs("12345"));
        assert!(!is_likely_mgrs(""));
        assert!(!is_likely_mgrs("38.8977"));
    }

    #[test]
    fn test_detect_mgrs_column_finds_correct_column() {
        let records = vec![
            csv::StringRecord::from(vec!["Name", "18SUJ2337006519", "Note"]),
            csv::StringRecord::from(vec!["Place", "33UUP0100010000", "Info"]),
        ];
        assert_eq!(detect_mgrs_column(&records), Some(1));
    }

    #[test]
    fn test_detect_mgrs_column_no_mgrs() {
        let records = vec![csv::StringRecord::from(vec!["Name", "Value", "Note"])];
        assert_eq!(detect_mgrs_column(&records), None);
    }

    #[test]
    fn test_detect_mgrs_column_empty() {
        let records: Vec<csv::StringRecord> = vec![];
        assert_eq!(detect_mgrs_column(&records), None);
    }

    #[test]
    fn test_detect_mgrs_column_first_column() {
        let records = vec![
            csv::StringRecord::from(vec!["18SUJ2337006519", "Washington DC"]),
            csv::StringRecord::from(vec!["33UUP0100010000", "Somewhere"]),
        ];
        assert_eq!(detect_mgrs_column(&records), Some(0));
    }
}
