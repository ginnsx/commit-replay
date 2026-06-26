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
    Remove(String),
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
            Some('-') => hunk.lines.push(HunkLine::Remove(line[1..].to_string())),
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

fn hunk_has_old_side(lines: &[HunkLine]) -> bool {
    lines
        .iter()
        .any(|hl| matches!(hl, HunkLine::Context(_) | HunkLine::Remove(_)))
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

    let start_idx = if hunk_has_old_side(&hunk.lines) {
        hunk.old_start.saturating_sub(1)
    } else {
        hunk.old_start.saturating_sub(1).min(lines.len())
    };

    if hunk_has_old_side(&hunk.lines) && start_idx > lines.len() {
        return Err(AppError::Vcs(format!(
            "patch context mismatch: hunk starts at line {} but file has {} lines",
            hunk.old_start,
            lines.len()
        )));
    }

    let mut out = lines[..start_idx].to_vec();
    let mut idx = start_idx;

    for hl in &hunk.lines {
        match hl {
            HunkLine::Context(expected) => {
                let actual = lines.get(idx).ok_or_else(|| {
                    AppError::Vcs(format!(
                        "patch context mismatch at line {}: expected {expected:?}, past end of file",
                        idx + 1
                    ))
                })?;
                if actual != expected {
                    return Err(AppError::Vcs(format!(
                        "patch context mismatch at line {}: expected {expected:?}, got {actual:?}",
                        idx + 1
                    )));
                }
                out.push(actual.clone());
                idx += 1;
            }
            HunkLine::Remove(expected) => {
                let actual = lines.get(idx).ok_or_else(|| {
                    AppError::Vcs(format!(
                        "patch context mismatch at line {}: expected removal of {expected:?}, past end of file",
                        idx + 1
                    ))
                })?;
                if actual != expected {
                    return Err(AppError::Vcs(format!(
                        "patch context mismatch at line {}: expected removal of {expected:?}, got {actual:?}",
                        idx + 1
                    )));
                }
                idx += 1;
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

fn find_subsequence(haystack: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn hunk_old_side_pattern(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|hl| match hl {
            HunkLine::Context(s) | HunkLine::Remove(s) => Some(s.clone()),
            HunkLine::Add(_) => None,
        })
        .collect()
}

fn hunk_remove_only_pattern(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|hl| match hl {
            HunkLine::Remove(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn hunk_new_side_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|hl| match hl {
            HunkLine::Context(s) | HunkLine::Add(s) => Some(s.clone()),
            HunkLine::Remove(_) => None,
        })
        .collect()
}

fn compress_line(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '"' && *c != '+' && *c != '\\')
        .collect::<String>()
        .to_lowercase()
}

fn find_line_exact(lines: &[String], needle: &str) -> Option<usize> {
    lines.iter().position(|l| l == needle)
}

fn find_line_trim(lines: &[String], needle: &str) -> Option<usize> {
    let n = needle.trim();
    lines.iter().position(|l| l.trim() == n)
}

fn find_line_loose(lines: &[String], needle: &str) -> Option<usize> {
    let n = compress_line(needle);
    if n.len() < 8 {
        return None;
    }
    lines.iter().position(|l| {
        let t = compress_line(l);
        t == n || (n.len() > 16 && t.contains(&n)) || (t.len() > 16 && n.contains(&t))
    })
}

fn find_line_fuzzy(lines: &[String], needle: &str) -> Option<usize> {
    find_line_exact(lines, needle)
        .or_else(|| find_line_trim(lines, needle))
        .or_else(|| find_line_loose(lines, needle))
}

pub fn diagnose_patch_against_base(patch: &str, base: &str) -> (usize, usize, usize) {
    let lines: Vec<String> = base.lines().map(String::from).collect();
    let mut total = 0usize;
    let mut matched = 0usize;
    for hunk in parse_hunks(patch).unwrap_or_default() {
        for hl in &hunk.lines {
            if let HunkLine::Remove(old) = hl {
                total += 1;
                if find_line_fuzzy(&lines, old).is_some() {
                    matched += 1;
                }
            }
        }
    }
    (total, matched, total.saturating_sub(matched))
}

fn apply_hunk_by_line_pairs(lines: &mut Vec<String>, hunk: &Hunk) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i < hunk.lines.len() {
        match &hunk.lines[i] {
            HunkLine::Remove(old) => {
                if let Some(pos) = find_line_fuzzy(lines, old) {
                    if i + 1 < hunk.lines.len() {
                        if let HunkLine::Add(new) = &hunk.lines[i + 1] {
                            lines[pos] = new.clone();
                            i += 2;
                            changed = true;
                            continue;
                        }
                    }
                    lines.remove(pos);
                    changed = true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    changed
}

fn find_subsequence_loose(haystack: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|window| {
        window.iter().zip(needle.iter()).all(|(a, b)| {
            let ca = compress_line(a);
            let cb = compress_line(b);
            ca == cb || (cb.len() > 16 && ca.contains(&cb)) || (ca.len() > 16 && cb.contains(&ca))
        })
    })
}

fn apply_hunk_by_search(lines: &mut Vec<String>, hunk: &Hunk) -> Result<()> {
    if hunk.old_count == 0 {
        for hl in &hunk.lines {
            if let HunkLine::Add(s) = hl {
                lines.push(s.clone());
            }
        }
        return Ok(());
    }

    let full_pattern = hunk_old_side_pattern(hunk);
    let pos = find_subsequence(lines, &full_pattern)
        .or_else(|| find_subsequence_loose(lines, &full_pattern))
        .or_else(|| {
            let removes = hunk_remove_only_pattern(hunk);
            if removes.is_empty() {
                None
            } else {
                find_subsequence(lines, &removes)
                    .or_else(|| find_subsequence_loose(lines, &removes))
            }
        });

    let Some(start) = pos else {
        return Err(AppError::Vcs("hunk old side not found in target".into()));
    };

    let matched_len = if find_subsequence(&lines[start..], &full_pattern).is_some() {
        full_pattern.len()
    } else {
        hunk_remove_only_pattern(hunk).len()
    };

    let replacement = hunk_new_side_lines(hunk);
    lines.splice(start..start + matched_len, replacement);
    Ok(())
}

/// Apply patches in order; each step tries strict then search-based apply.
pub fn apply_patches_sequential(base: &str, patches: &[&str]) -> Option<String> {
    let mut current = base.to_string();
    let mut any = false;
    for patch in patches {
        let Some(next) = apply_unified_patch(Some(&current), patch)
            .or_else(|_| apply_unified_patch_by_search(&current, patch))
            .ok()
        else {
            continue;
        };
        if next != current {
            any = true;
            current = next;
        }
    }
    if any {
        Some(current)
    } else {
        None
    }
}

/// Best-effort target `after` from optional merged patch and/or per-commit patches.
pub fn resolve_target_after(
    before: &str,
    merged_patch: Option<&str>,
    sequential_patches: &[&str],
) -> Option<String> {
    if !sequential_patches.is_empty() {
        if let Some(next) = apply_patches_sequential(before, sequential_patches) {
            return Some(next);
        }
    }
    let patch = merged_patch?;
    apply_unified_patch(Some(before), patch)
        .or_else(|_| apply_unified_patch_by_search(before, patch))
        .ok()
        .filter(|next| next != before)
}

/// Apply patch by searching for hunk old-side content in `base` (cross-version / line drift).
pub fn apply_unified_patch_by_search(base: &str, patch: &str) -> Result<String> {
    let mut lines: Vec<String> = base.lines().map(String::from).collect();
    let hunks = parse_hunks(patch)?;
    let mut any = false;
    for hunk in hunks {
        if apply_hunk_by_search(&mut lines, &hunk).is_ok() {
            any = true;
        } else if apply_hunk_by_line_pairs(&mut lines, &hunk) {
            any = true;
        }
    }
    if any {
        Ok(lines.join("\n"))
    } else {
        Err(AppError::Vcs("no hunk applied to target".into()))
    }
}

#[derive(Debug, Clone)]
struct RawHunk {
    header: String,
    body: Vec<String>,
    old_start: usize,
    old_count: usize,
}

fn parse_raw_hunks(patch: &str) -> Result<Vec<RawHunk>> {
    let mut hunks = Vec::new();
    let mut header: Option<String> = None;
    let mut body: Vec<String> = Vec::new();

    let flush = |header: &mut Option<String>, body: &mut Vec<String>, hunks: &mut Vec<RawHunk>| {
        if let Some(h) = header.take() {
            let (old_start, old_count) = parse_hunk_header_range(&h)?;
            hunks.push(RawHunk {
                header: h,
                body: std::mem::take(body),
                old_start,
                old_count,
            });
        }
        Ok::<(), AppError>(())
    };

    for line in patch.lines() {
        if line.starts_with("@@") {
            flush(&mut header, &mut body, &mut hunks)?;
            header = Some(line.to_string());
            continue;
        }
        if header.is_some()
            && !line.starts_with("--- ")
            && !line.starts_with("+++ ")
            && !line.starts_with("Index:")
        {
            body.push(line.to_string());
        }
    }
    flush(&mut header, &mut body, &mut hunks)?;
    Ok(hunks)
}

fn parse_hunk_header_range(header: &str) -> Result<(usize, usize)> {
    let parts: Vec<&str> = header.split_whitespace().collect();
    let old_part = parts
        .iter()
        .find(|p| p.starts_with('-'))
        .ok_or_else(|| AppError::Vcs(format!("invalid hunk header: {header}")))?;
    Ok(parse_range(old_part.trim_start_matches('-')))
}

fn old_ranges_overlap(a: &RawHunk, b: &RawHunk) -> bool {
    if a.old_count == 0 || b.old_count == 0 {
        return a.old_count == 0 && b.old_count == 0;
    }
    let a_end = a.old_start.saturating_add(a.old_count.saturating_sub(1));
    let b_end = b.old_start.saturating_add(b.old_count.saturating_sub(1));
    a.old_start <= b_end && b.old_start <= a_end
}

fn hunk_to_text(h: &RawHunk) -> String {
    if h.body.is_empty() {
        h.header.clone()
    } else {
        format!("{}\n{}", h.header, h.body.join("\n"))
    }
}

/// Merge patches in commit order; later hunks replace overlapping earlier ones (by old line range).
pub fn merge_patches_last_wins(patches: &[&str]) -> Option<String> {
    let mut merged: Vec<RawHunk> = Vec::new();
    for patch in patches {
        let Ok(hunks) = parse_raw_hunks(patch) else {
            continue;
        };
        for hunk in hunks {
            merged.retain(|h| !old_ranges_overlap(h, &hunk));
            merged.push(hunk);
        }
    }
    if merged.is_empty() {
        return None;
    }
    Some(merged.iter().map(hunk_to_text).collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_single_hunk() {
        let base = "a\nold\nc";
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

    #[test]
    fn rejects_context_mismatch_at_hunk_line() {
        let base = "line1\nline2\nline3\n";
        let patch = "@@ -3,1 +3,1 @@\n-wrong line\n+new line\n";
        assert!(apply_unified_patch(Some(base), patch).is_err());
    }

    #[test]
    fn applies_when_context_matches() {
        let base = "line1\nline2\nline3\n";
        let patch = "@@ -3,1 +3,1 @@\n-line3\n+new line\n";
        let out = apply_unified_patch(Some(base), patch).unwrap();
        assert_eq!(out, "line1\nline2\nnew line");
    }

    #[test]
    fn apply_by_search_finds_remove_line_at_different_offset() {
        let old_line = "                                          ng-model=\"formParams.description\" style=\"height: 50px\">";
        let new_line = "                                          ng-model=\"formParams.description\" rows=\"5\">";
        let base = format!("<div>\n<textarea\n{old_line}\n</textarea>\n</div>\n");
        let patch = format!("@@ -3,1 +3,1 @@\n-{old_line}\n+{new_line}\n");
        let out = apply_unified_patch_by_search(&base, &patch).unwrap();
        assert!(out.contains("rows=\"5\""));
        assert!(!out.contains("style=\"height: 50px\""));
    }

    #[test]
    fn apply_patches_sequential_cross_version() {
        let old_line = "                                          ng-model=\"formParams.description\" style=\"height: 50px\">";
        let new_line = "                                          ng-model=\"formParams.description\" rows=\"5\">";
        let base = format!("<div>\n<textarea\n{old_line}\n</textarea>\n</div>\n");
        let p1 = format!("@@ -3,1 +3,1 @@\n-{old_line}\n+{new_line}\n");
        let out = apply_patches_sequential(&base, &[&p1]).unwrap();
        assert!(out.contains("rows=\"5\""));
        assert!(!out.contains("style=\"height: 50px\""));
    }

    #[test]
    fn merge_patches_last_wins_replaces_overlapping_hunk() {
        let p1 = "@@ -1,3 +1,3 @@\n a\n-old1\n+new1\n c\n";
        let p2 = "@@ -1,3 +1,3 @@\n a\n-old1\n+new2\n c\n";
        let merged = merge_patches_last_wins(&[p1, p2]).unwrap();
        assert!(!merged.contains("+new1"));
        assert!(merged.contains("+new2"));
    }
}
