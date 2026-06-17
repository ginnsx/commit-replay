use similar::{ChangeTag, TextDiff};

use crate::store::models::{DiffLine, DiffLineType};

const MAX_DIFF_LINES: usize = 5000;

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn patch_to_diff_lines(patch: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let mut in_hunk = false;
    let mut old_ln = 1u32;
    let mut new_ln = 1u32;

    for line in patch.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
            if let Some((old_start, new_start)) = parse_hunk_starts(line) {
                old_ln = old_start;
                new_ln = new_start;
            }
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with("+++ ")
            || line.starts_with("--- ")
            || line.starts_with("Index:")
            || line.starts_with('\\')
        {
            continue;
        }
        if out.len() >= MAX_DIFF_LINES {
            break;
        }
        match line.chars().next() {
            Some(' ') => {
                let text = line[1..].to_string();
                out.push(DiffLine {
                    line_type: DiffLineType::Ctx,
                    old: Some(old_ln),
                    new: Some(new_ln),
                    text,
                });
                old_ln += 1;
                new_ln += 1;
            }
            Some('-') => {
                out.push(DiffLine {
                    line_type: DiffLineType::Del,
                    old: Some(old_ln),
                    new: None,
                    text: line[1..].to_string(),
                });
                old_ln += 1;
            }
            Some('+') => {
                out.push(DiffLine {
                    line_type: DiffLineType::Add,
                    old: None,
                    new: Some(new_ln),
                    text: line[1..].to_string(),
                });
                new_ln += 1;
            }
            _ => {}
        }
    }
    out
}

fn parse_hunk_starts(line: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let old_part = parts.iter().find(|p| p.starts_with('-'))?;
    let new_part = parts.iter().find(|p| p.starts_with('+'))?;
    let old_start = parse_range_start(old_part.trim_start_matches('-'));
    let new_start = parse_range_start(new_part.trim_start_matches('+'));
    Some((old_start.max(1), new_start.max(1)))
}

fn parse_range_start(range: &str) -> u32 {
    range
        .split(',')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

pub fn count_patch_stats(patch: &str) -> (u32, u32) {
    count_line_stats(&patch_to_diff_lines(patch))
}

pub fn lines_to_diff(before: Option<&str>, after: Option<&str>) -> Vec<DiffLine> {
    let before_text = normalize_newlines(before.unwrap_or(""));
    let after_text = normalize_newlines(after.unwrap_or(""));
    let diff = TextDiff::from_lines(&before_text, &after_text);
    let mut out = Vec::new();
    let mut old_ln = 1u32;
    let mut new_ln = 1u32;

    for change in diff.iter_all_changes() {
        if out.len() >= MAX_DIFF_LINES {
            break;
        }
        let text = change.value().trim_end_matches('\n').to_string();
        match change.tag() {
            ChangeTag::Equal => {
                out.push(DiffLine {
                    line_type: DiffLineType::Ctx,
                    old: Some(old_ln),
                    new: Some(new_ln),
                    text,
                });
                old_ln += 1;
                new_ln += 1;
            }
            ChangeTag::Delete => {
                out.push(DiffLine {
                    line_type: DiffLineType::Del,
                    old: Some(old_ln),
                    new: None,
                    text,
                });
                old_ln += 1;
            }
            ChangeTag::Insert => {
                out.push(DiffLine {
                    line_type: DiffLineType::Add,
                    old: None,
                    new: Some(new_ln),
                    text,
                });
                new_ln += 1;
            }
        }
    }
    out
}

pub fn count_line_stats(diff: &[DiffLine]) -> (u32, u32) {
    let mut additions = 0u32;
    let mut deletions = 0u32;
    for line in diff {
        match line.line_type {
            DiffLineType::Add => additions += 1,
            DiffLineType::Del => deletions += 1,
            DiffLineType::Ctx => {}
        }
    }
    (additions, deletions)
}

pub fn align_conflict_lines(before: &[String], after: &[String]) -> Vec<(Option<String>, Option<String>, &'static str)> {
    let max = before.len().max(after.len());
    let mut rows = Vec::with_capacity(max);
    for i in 0..max {
        let left = before.get(i).cloned();
        let right = after.get(i).cloned();
        let kind = match (&left, &right) {
            (Some(l), Some(r)) if l == r => "same",
            (None, Some(_)) => "add",
            (Some(_), None) => "del",
            _ => "chg",
        };
        rows.push((left, right, kind));
    }
    rows
}

pub fn find_overlap_lines(before: &[String], after: &[String]) -> Option<[u32; 2]> {
    let rows = align_conflict_lines(before, after);
    let mut start: Option<u32> = None;
    let mut end = 0u32;
    let mut result: Option<[u32; 2]> = None;
    for (i, (_, _, kind)) in rows.iter().enumerate() {
        if *kind == "chg" {
            let line = (i + 1) as u32;
            if start.is_none() {
                start = Some(line);
            }
            end = line;
        } else if let Some(s) = start {
            result = Some([s, end]);
            start = None;
        }
    }
    if let Some(s) = start {
        result = Some([s, end]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::models::DiffLineType;

    #[test]
    fn diff_counts_add_del() {
        let lines = lines_to_diff(Some("a\n"), Some("a\nb\n"));
        let (add, del) = count_line_stats(&lines);
        assert!(add >= 1);
        assert_eq!(del, 0);
    }

    #[test]
    fn patch_to_diff_marks_single_line_change() {
        let patch = "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n";
        let lines = patch_to_diff_lines(patch);
        assert!(lines.iter().any(|l| matches!(l.line_type, DiffLineType::Del) && l.text == "old line"));
        assert!(lines.iter().any(|l| matches!(l.line_type, DiffLineType::Add) && l.text == "new line"));
        assert!(lines.iter().any(|l| matches!(l.line_type, DiffLineType::Ctx) && l.text == "unchanged"));
    }

    #[test]
    fn lines_to_diff_handles_crlf() {
        let lines = lines_to_diff(Some("a\r\nb\r\n"), Some("a\nb\nc\n"));
        let (add, del) = count_line_stats(&lines);
        assert_eq!(del, 0);
        assert_eq!(add, 1);
    }
}
