use crate::error::{AppError, Result};

/// Apply a unified diff patch to `base`. For `None` base (new file), treats base as empty.
pub fn apply_unified_patch(base: Option<&str>, patch: &str) -> Result<String> {
    let mut lines: Vec<String> = base.unwrap_or("").lines().map(String::from).collect();
    let hunks = parse_hunks(patch)?;

    for hunk in hunks.iter().rev() {
        apply_hunk(&mut lines, hunk)?;
    }

    Ok(lines.join("\n"))
}

/// Reconstruct pre-change file content from unified diff hunks.
pub fn reconstruct_old_from_patch(patch: &str) -> String {
    collect_patch_side(patch, true)
}

/// Reconstruct post-change file content from unified diff hunks.
pub fn reconstruct_new_from_patch(patch: &str) -> String {
    collect_patch_side(patch, false)
}

fn collect_patch_side(patch: &str, old_side: bool) -> String {
    let mut out = Vec::new();
    let mut in_hunk = false;
    for line in patch.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
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
        match line.chars().next() {
            Some(' ') => out.push(line[1..].to_string()),
            Some('-') if old_side => out.push(line[1..].to_string()),
            Some('+') if !old_side => out.push(line[1..].to_string()),
            _ => {}
        }
    }
    out.join("\n")
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug)]
enum HunkLine {
    Context(String),
    Remove(()),
    Add(String),
}

fn parse_hunks(patch: &str) -> Result<Vec<Hunk>> {
    let mut hunks = Vec::new();
    let mut current: Option<Hunk> = None;

    for line in patch.lines() {
        if line.starts_with("@@") {
            if let Some(h) = current.take() {
                hunks.push(h);
            }
            current = Some(parse_hunk_header(line)?);
            continue;
        }
        let Some(ref mut hunk) = current else {
            continue;
        };
        if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with("Index:") {
            continue;
        }
        if line.starts_with('\\') {
            continue;
        }
        match line.chars().next() {
            Some(' ') => hunk.lines.push(HunkLine::Context(line[1..].to_string())),
            Some('+') => hunk.lines.push(HunkLine::Add(line[1..].to_string())),
            Some('-') => hunk.lines.push(HunkLine::Remove(())),
            _ => {}
        }
    }
    if let Some(h) = current {
        hunks.push(h);
    }
    Ok(hunks)
}

fn parse_hunk_header(line: &str) -> Result<Hunk> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let old_part = parts
        .iter()
        .find(|p| p.starts_with('-'))
        .ok_or_else(|| AppError::Vcs(format!("invalid hunk header: {line}")))?;
    let (old_start, old_count) = parse_range(old_part.trim_start_matches('-'));

    Ok(Hunk {
        old_start,
        old_count,
        lines: Vec::new(),
    })
}

/// Parse `start,count` from hunk range (count defaults to 1; `0,0` means zero old lines).
fn parse_range(range: &str) -> (usize, usize) {
    let mut parts = range.split(',');
    let start = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let count = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (start, count)
}

fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) -> Result<()> {
    // @@ -0,0 +1,N @@ — pure addition, ignore old line numbers.
    if hunk.old_count == 0 {
        for hl in &hunk.lines {
            if let HunkLine::Add(s) = hl {
                lines.push(s.clone());
            }
        }
        return Ok(());
    }

    let start_idx = hunk.old_start.saturating_sub(1).min(lines.len());
    let mut out = lines[..start_idx].to_vec();
    let mut idx = start_idx;

    for hl in &hunk.lines {
        match hl {
            HunkLine::Context(s) => {
                if idx < lines.len() {
                    out.push(lines[idx].clone());
                    idx += 1;
                } else {
                    out.push(s.clone());
                }
            }
            HunkLine::Remove(_) => {
                if idx < lines.len() {
                    idx += 1;
                }
            }
            HunkLine::Add(s) => {
                out.push(s.clone());
            }
        }
    }

    out.extend_from_slice(&lines[idx..]);
    *lines = out;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_single_hunk() {
        let base = "a\nb\nc";
        let patch = "@@ -1,3 +1,3 @@\n a\n-old\n+new\n c\n";
        let out = apply_unified_patch(Some(base), patch).unwrap();
        assert_eq!(out, "a\nnew\nc");
    }

    #[test]
    fn creates_from_empty_base() {
        let patch = "@@ -0,0 +1,2 @@\n+line1\n+line2\n";
        let out = apply_unified_patch(None, patch).unwrap();
        assert_eq!(out, "line1\nline2");
    }

    #[test]
    fn large_old_start_on_empty_base_does_not_panic() {
        let patch = "@@ -2576,5 +2576,6 @@\n+only new line\n";
        let out = apply_unified_patch(None, patch).unwrap();
        assert_eq!(out, "only new line");
    }

    #[test]
    fn reconstruct_old_and_new_from_modify_patch() {
        let patch = "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n";
        assert_eq!(
            reconstruct_old_from_patch(patch),
            "# Project\nold line\nunchanged"
        );
        assert_eq!(
            reconstruct_new_from_patch(patch),
            "# Project\nnew line\nunchanged"
        );
    }

    #[test]
    fn apply_falls_back_to_patch_base_when_target_differs() {
        let patch = "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n";
        let target_before = "totally\nunrelated\ncontent\n";
        let old = reconstruct_old_from_patch(patch);
        let after = apply_unified_patch(Some(&old), patch).unwrap();
        assert_eq!(after, reconstruct_new_from_patch(patch));
        assert_ne!(after, target_before);
    }
}
