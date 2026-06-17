use crate::error::Result;
use crate::model::ReplayUnitMeta;

use super::ref_util::format_git_ref;

const FIELD_SEP: char = '\x00';

pub fn parse_git_log(output: &str) -> Result<Vec<ReplayUnitMeta>> {
    let mut entries = Vec::new();
    let mut current: Option<(String, String, String, String)> = None;
    let mut file_count = 0usize;

    let flush = |entries: &mut Vec<ReplayUnitMeta>,
                     current: &mut Option<(String, String, String, String)>,
                     file_count: &mut usize| {
        if let Some((sha, author, date, message)) = current.take() {
            entries.push(ReplayUnitMeta {
                source_ref: format_git_ref(&sha),
                author,
                date,
                message,
                changed_paths_count: *file_count,
            });
            *file_count = 0;
        }
    };

    for line in output.lines() {
        if line.contains(FIELD_SEP) {
            flush(&mut entries, &mut current, &mut file_count);
            let parts: Vec<&str> = line.split(FIELD_SEP).collect();
            if parts.len() >= 4 && !parts[0].is_empty() {
                current = Some((
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                ));
            }
            continue;
        }
        if line.is_empty() {
            flush(&mut entries, &mut current, &mut file_count);
            continue;
        }
        if is_numstat_line(line) {
            file_count += 1;
        } else if line.contains("files changed") {
            let count = shortstat_file_count(line);
            if current.is_some() {
                file_count = count;
            } else if let Some(last) = entries.last_mut() {
                if last.changed_paths_count == 0 {
                    last.changed_paths_count = count;
                }
            }
        }
    }

    flush(&mut entries, &mut current, &mut file_count);
    Ok(entries)
}

fn is_numstat_line(line: &str) -> bool {
    let mut parts = line.split('\t');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), Some(c)) if !a.is_empty() && !b.is_empty() && !c.is_empty()
    )
}

fn shortstat_file_count(line: &str) -> usize {
    line.split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_entry() {
        let output = "abc123def456\0Alice\02026-01-01T12:00:00+00:00\0Fix bug\0\n\n 2 files changed, 3 insertions(+), 1 deletion(-)\n";
        let entries = parse_git_log(output).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_ref, "git:abc123def456");
        assert_eq!(entries[0].author, "Alice");
        assert_eq!(entries[0].message, "Fix bug");
        assert_eq!(entries[0].changed_paths_count, 2);
    }

    #[test]
    fn parses_numstat_lines() {
        let output = "sha1\0author\02026-01-01\0msg\0\n10\t5\tsrc/a.rs\n3\t1\tsrc/b.rs\n\n";
        let entries = parse_git_log(output).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].changed_paths_count, 2);
    }

    #[test]
    fn parses_multiple_entries() {
        let output = "aaa\0a1\02026-01-01\0first\0\n\nbbb\0a2\02026-01-02\0second\0\n1\t0\tf.txt\n\n";
        let entries = parse_git_log(output).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "first");
        assert_eq!(entries[1].message, "second");
        assert_eq!(entries[1].changed_paths_count, 1);
    }
}
