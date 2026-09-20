use crate::{
    model::{ChangeAnalysis, FileChange, FileChangeKind, LocationStatus, MatchMethod, MergeStatus},
    preview::patch_apply::{reconstruct_new_from_patch, reconstruct_old_from_patch},
};

pub fn analyze_file_change(fc: &FileChange) -> Option<ChangeAnalysis> {
    if fc.target_path.is_none() {
        return Some(analysis(
            fc,
            LocationStatus::PathUnmapped,
            MatchMethod::None,
            0.0,
            0,
            MergeStatus::Conflict,
            "目标路径未映射",
        ));
    }
    if !matches!(fc.kind, FileChangeKind::Modify | FileChangeKind::Rename) {
        return None;
    }

    let patch = fc.patch.as_deref()?;
    let base = reconstruct_old_from_patch(patch);
    let theirs = reconstruct_new_from_patch(patch);
    if base.is_empty() && theirs.is_empty() {
        return None;
    }
    let Some(ours) = fc.before.as_deref() else {
        return Some(analysis(
            fc,
            LocationStatus::FileMissing,
            MatchMethod::None,
            0.0,
            0,
            MergeStatus::Conflict,
            "目标文件不存在，无法应用修改",
        ));
    };

    let base_lines = norm_lines(&base);
    let theirs_lines = norm_lines(&theirs);
    let ours_lines = norm_lines(ours);
    let expected_start = first_hunk_old_start(patch).unwrap_or(1).saturating_sub(1);

    if !theirs_lines.is_empty() {
        let theirs_count = count_subsequence(&ours_lines, &theirs_lines);
        if theirs_count > 0 {
            return Some(analysis(
                fc,
                LocationStatus::AlreadyContains,
                MatchMethod::None,
                1.0,
                theirs_count,
                MergeStatus::Skip,
                "目标已包含期望内容",
            ));
        }
    }

    if base_lines == theirs_lines {
        let base_count = count_subsequence(&ours_lines, &base_lines);
        let status = if base_count == 1 {
            LocationStatus::Exact
        } else {
            LocationStatus::ContextDrift
        };
        return Some(analysis(
            fc,
            status,
            MatchMethod::None,
            1.0,
            base_count,
            MergeStatus::KeepTarget,
            "源提交没有净变更，保留目标内容",
        ));
    }

    let exact_positions = find_subsequence_positions(&ours_lines, &base_lines);
    if exact_positions.len() > 1 {
        return Some(analysis(
            fc,
            LocationStatus::MultipleCandidates,
            MatchMethod::OldBlock,
            0.5,
            exact_positions.len(),
            MergeStatus::Conflict,
            "目标中存在多个相同候选位置",
        ));
    }
    if let Some(pos) = exact_positions.first().copied() {
        let (status, method, merge, reason) = if pos == expected_start {
            (
                LocationStatus::Exact,
                MatchMethod::OldBlock,
                MergeStatus::AutoApply,
                "源旧内容在目标中完整匹配",
            )
        } else {
            (
                LocationStatus::ContextDrift,
                MatchMethod::Context,
                MergeStatus::AutoMerge,
                "源旧内容可定位，但行号已漂移",
            )
        };
        return Some(analysis(fc, status, method, 1.0, 1, merge, reason));
    }

    let context_candidates = find_context_candidates(&ours_lines, &base_lines, &theirs_lines);
    if context_candidates > 1 {
        return Some(analysis(
            fc,
            LocationStatus::MultipleCandidates,
            MatchMethod::Context,
            0.5,
            context_candidates,
            MergeStatus::Conflict,
            "目标中存在多个上下文候选位置",
        ));
    }
    if context_candidates == 1 {
        return Some(analysis(
            fc,
            LocationStatus::SameRegionConflict,
            MatchMethod::Context,
            0.75,
            1,
            MergeStatus::Conflict,
            "目标有独立修改，与迁入变更区域重叠",
        ));
    }

    let fuzzy_count = count_fuzzy_block(&ours_lines, &base_lines);
    if fuzzy_count > 1 {
        return Some(analysis(
            fc,
            LocationStatus::MultipleCandidates,
            MatchMethod::Fuzzy,
            0.4,
            fuzzy_count,
            MergeStatus::Conflict,
            "目标中存在多个模糊候选位置",
        ));
    }
    if fuzzy_count == 1 {
        return Some(analysis(
            fc,
            LocationStatus::ContextDrift,
            MatchMethod::Fuzzy,
            0.6,
            1,
            MergeStatus::AutoMerge,
            "源旧内容可通过模糊匹配定位",
        ));
    }

    Some(analysis(
        fc,
        LocationStatus::NotFound,
        MatchMethod::None,
        0.0,
        0,
        MergeStatus::Conflict,
        "无法在目标中定位源旧内容",
    ))
}

fn analysis(
    fc: &FileChange,
    location_status: LocationStatus,
    match_method: MatchMethod,
    confidence: f32,
    candidate_count: usize,
    merge_status: MergeStatus,
    reason: &str,
) -> ChangeAnalysis {
    ChangeAnalysis {
        target_path: fc.target_path.clone(),
        location_status,
        match_method,
        confidence,
        candidate_count,
        merge_status,
        reason: reason.into(),
    }
}

fn norm_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::to_string)
        .collect()
}

fn first_hunk_old_start(patch: &str) -> Option<usize> {
    let header = patch.lines().find(|line| line.starts_with("@@"))?;
    let old_range = header
        .split_whitespace()
        .find(|part| part.starts_with('-'))?;
    old_range
        .trim_start_matches('-')
        .split(',')
        .next()
        .and_then(|s| s.parse().ok())
}

fn find_subsequence_positions(haystack: &[String], needle: &[String]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(idx, window)| (window == needle).then_some(idx))
        .collect()
}

fn count_subsequence(haystack: &[String], needle: &[String]) -> usize {
    find_subsequence_positions(haystack, needle).len()
}

fn changed_span(base: &[String], theirs: &[String]) -> Option<(usize, usize)> {
    let max = base.len().max(theirs.len());
    let first = (0..max).find(|idx| base.get(*idx) != theirs.get(*idx))?;
    let last = (0..max)
        .rev()
        .find(|idx| base.get(*idx) != theirs.get(*idx))
        .unwrap_or(first);
    Some((first, last))
}

fn find_context_candidates(ours: &[String], base: &[String], theirs: &[String]) -> usize {
    let Some((first_changed, last_changed)) = changed_span(base, theirs) else {
        return 0;
    };
    let prefix = &base[..first_changed];
    let suffix = if last_changed + 1 < base.len() {
        &base[last_changed + 1..]
    } else {
        &[]
    };
    if prefix.is_empty() && suffix.is_empty() {
        return 0;
    }

    let mut count = 0usize;
    for start in 0..ours.len() {
        if !prefix.is_empty() {
            let end = start + prefix.len();
            if end > ours.len() || &ours[start..end] != prefix {
                continue;
            }
        }

        if suffix.is_empty() {
            count += 1;
            continue;
        }

        let suffix_start_min = start + prefix.len();
        for suffix_start in suffix_start_min..=ours.len().saturating_sub(suffix.len()) {
            if &ours[suffix_start..suffix_start + suffix.len()] == suffix {
                count += 1;
                break;
            }
        }
    }
    count
}

fn compact_line(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn count_fuzzy_block(ours: &[String], base: &[String]) -> usize {
    if base.is_empty() || ours.len() < base.len() {
        return 0;
    }
    let compact_base = base.iter().map(|s| compact_line(s)).collect::<Vec<_>>();
    if compact_base.iter().any(|s| s.len() < 4) {
        return 0;
    }
    ours.windows(base.len())
        .filter(|window| {
            window
                .iter()
                .map(|s| compact_line(s))
                .zip(compact_base.iter())
                .all(|(a, b)| a == *b)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ConflictRisk, FileChange, FileChangeKind, LocationStatus, MatchMethod, MergeStatus,
    };

    fn modify_fc(before: Option<&str>, patch: &str) -> FileChange {
        FileChange {
            path: "src/a.txt".into(),
            target_path: Some("src/a.txt".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: before.map(str::to_string),
            after: None,
            source_after: None,
            after_bytes: None,
            source_ref: None,
            patch: Some(patch.into()),
            conflict_risk: Some(ConflictRisk::Low),
            analysis: None,
        }
    }

    #[test]
    fn old_block_exact_match_auto_applies() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let analysis = analyze_file_change(&modify_fc(Some("alpha\nold\nomega\n"), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::Exact);
        assert_eq!(analysis.match_method, MatchMethod::OldBlock);
        assert_eq!(analysis.merge_status, MergeStatus::AutoApply);
        assert_eq!(analysis.candidate_count, 1);
    }

    #[test]
    fn line_drift_with_context_auto_merges_for_review() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let before = "header\nalpha\nold\nomega\nfooter\n";
        let analysis = analyze_file_change(&modify_fc(Some(before), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::ContextDrift);
        assert_eq!(analysis.match_method, MatchMethod::Context);
        assert_eq!(analysis.merge_status, MergeStatus::AutoMerge);
        assert_eq!(analysis.candidate_count, 1);
    }

    #[test]
    fn target_already_contains_theirs_skips() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let analysis = analyze_file_change(&modify_fc(Some("alpha\nnew\nomega\n"), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::AlreadyContains);
        assert_eq!(analysis.merge_status, MergeStatus::Skip);
        assert_eq!(analysis.match_method, MatchMethod::None);
    }

    #[test]
    fn repeated_old_block_reports_multiple_candidates() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let before = "alpha\nold\nomega\nbetween\nalpha\nold\nomega\n";
        let analysis = analyze_file_change(&modify_fc(Some(before), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::MultipleCandidates);
        assert_eq!(analysis.merge_status, MergeStatus::Conflict);
        assert_eq!(analysis.candidate_count, 2);
    }

    #[test]
    fn missing_old_block_reports_not_found() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let analysis =
            analyze_file_change(&modify_fc(Some("unrelated\ncontent\n"), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::NotFound);
        assert_eq!(analysis.merge_status, MergeStatus::Conflict);
    }

    #[test]
    fn missing_target_file_for_modify_blocks() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let analysis = analyze_file_change(&modify_fc(None, patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::FileMissing);
        assert_eq!(analysis.merge_status, MergeStatus::Conflict);
    }

    #[test]
    fn same_region_target_edit_blocks() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n";
        let analysis =
            analyze_file_change(&modify_fc(Some("alpha\ntarget\nomega\n"), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::SameRegionConflict);
        assert_eq!(analysis.merge_status, MergeStatus::Conflict);
    }

    #[test]
    fn source_noop_keeps_target_change() {
        let patch = "@@ -1,3 +1,3 @@\n alpha\n old\n omega\n";
        let analysis =
            analyze_file_change(&modify_fc(Some("alpha\ntarget\nomega\n"), patch)).unwrap();

        assert_eq!(analysis.merge_status, MergeStatus::KeepTarget);
    }

    #[test]
    fn crlf_no_final_newline_and_chinese_are_normalized() {
        let patch = "@@ -1,2 +1,2 @@\n 中文\n-old\n+new\n";
        let analysis = analyze_file_change(&modify_fc(Some("中文\r\nold"), patch)).unwrap();

        assert_eq!(analysis.location_status, LocationStatus::Exact);
        assert_eq!(analysis.merge_status, MergeStatus::AutoApply);
    }
}
