use crate::error::{AppError, Result};

/// Apply a unified diff patch to `base`. For `None` base (new file), treats base as empty.
pub fn apply_unified_patch(base: Option<&str>, patch: &str) -> Result<String> {
    let mut lines: Vec<String> = base.unwrap_or("").lines().map(String::from).collect();
    let hunks = parse_hunks(patch)?;

    // Apply from bottom to top so line indices stay valid.
    for hunk in hunks.iter().rev() {
        apply_hunk(&mut lines, hunk)?;
    }

    Ok(lines.join("\n"))
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
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
    // @@ -1,3 +1,3 @@
    let parts: Vec<&str> = line.split_whitespace().collect();
    let old_part = parts
        .iter()
        .find(|p| p.starts_with('-'))
        .ok_or_else(|| AppError::Vcs(format!("invalid hunk header: {line}")))?;
    let old_range = old_part.trim_start_matches('-');
    let old_start = old_range
        .split(',')
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    Ok(Hunk {
        old_start,
        lines: Vec::new(),
    })
}

fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) -> Result<()> {
    let mut out: Vec<String> = lines[..hunk.old_start.saturating_sub(1)].to_vec();
    let mut idx = hunk.old_start.saturating_sub(1);

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
}
