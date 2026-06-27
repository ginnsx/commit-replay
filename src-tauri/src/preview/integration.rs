use std::collections::HashMap;

use crate::{
    diff::line_diff::{find_overlap_lines, lines_to_diff, patch_to_diff_lines},
    model::{FileChange, FileChangeKind, LocationStatus, MergeStatus},
    preview::patch_apply::{reconstruct_new_from_patch, reconstruct_old_from_patch},
    preview::target_wc::{check_apply, TargetWcKind},
    relay::file_kind_to_status,
    store::models::{
        DiffLineType, IntegrationItemView, IntegrationPlanResult, IntegrationStatus,
        IntegrationStrategy, MigrationMode,
    },
};

fn normalize_content(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn content_equal(a: Option<&str>, b: Option<&str>) -> bool {
    normalize_content(a.unwrap_or("")) == normalize_content(b.unwrap_or(""))
}

fn content_equal_ignoring_final_newline(a: Option<&str>, b: Option<&str>) -> bool {
    let norm = |s: &str| {
        normalize_content(s)
            .trim_end_matches('\n')
            .to_string()
    };
    norm(a.unwrap_or("")) == norm(b.unwrap_or(""))
}

fn to_lines(text: Option<&str>) -> Vec<String> {
    text.unwrap_or("")
        .lines()
        .map(str::to_string)
        .collect()
}

fn patch_applies_cleanly(target_kind: TargetWcKind, wc_root: &str, fc: &FileChange) -> bool {
    matches!(
        check_apply(target_kind, wc_root, fc),
        crate::model::ConflictRisk::Low
    )
}

fn line_match_ratio(a: &str, b: &str) -> f32 {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    let max = la.len().max(lb.len());
    if max == 0 {
        return 1.0;
    }
    let same = (0..max).filter(|i| la.get(*i) == lb.get(*i)).count();
    same as f32 / max as f32
}

fn target_has_expected_content(fc: &FileChange) -> bool {
    if let Some(patch) = fc.patch.as_deref() {
        let expected = reconstruct_new_from_patch(patch);
        if !expected.is_empty()
            && content_equal_ignoring_final_newline(fc.before.as_deref(), Some(&expected))
        {
            return true;
        }
    } else if content_equal(fc.before.as_deref(), fc.after.as_deref()) {
        return true;
    }
    if fc.conflict_risk == Some(crate::model::ConflictRisk::High) {
        return false;
    }
    let Some(patch) = fc.patch.as_deref() else {
        return false;
    };
    let expected = reconstruct_new_from_patch(patch);
    match fc.kind {
        FileChangeKind::Delete => fc.before.is_none(),
        FileChangeKind::Binary => false,
        _ => !expected.is_empty() && content_equal(fc.before.as_deref(), Some(&expected)),
    }
}

fn has_meaningful_overlap(
    target_before: Option<&str>,
    source_before: Option<&str>,
    source_after: Option<&str>,
) -> bool {
    if content_equal(target_before, source_after) {
        return false;
    }
    let Some(sb) = source_before.filter(|s| !s.is_empty()) else {
        return false;
    };
    let target = target_before.unwrap_or("");
    if content_equal(Some(target), Some(sb)) {
        return false;
    }
    // Unrelated baselines (cross-version) — not a line-level merge conflict.
    if line_match_ratio(target, sb) < 0.5 {
        return false;
    }
    let sb_lines: Vec<&str> = sb.lines().collect();
    let target_lines: Vec<&str> = target.lines().collect();
    let after_lines: Vec<&str> = source_after.unwrap_or("").lines().collect();
    let max = sb_lines.len().max(target_lines.len()).max(after_lines.len());
    for i in 0..max {
        let b = sb_lines.get(i).copied();
        let t = target_lines.get(i).copied();
        let a = after_lines.get(i).copied();
        let target_changed = b != t;
        let incoming_changed = b != a;
        if target_changed && incoming_changed {
            return true;
        }
    }
    false
}

fn integration_diff_lines(fc: &FileChange) -> Vec<crate::store::models::DiffLine> {
    let lines = if let Some(patch) = fc.patch.as_deref() {
        patch_to_diff_lines(patch)
    } else {
        lines_to_diff(fc.before.as_deref(), fc.after.as_deref())
    };
    lines
        .into_iter()
        .filter(|l| !matches!(l.line_type, DiffLineType::Ctx))
        .collect()
}

pub fn build_integration_plan(
    files: &[FileChange],
    target_wc_path: &str,
    target_kind: TargetWcKind,
    mode: MigrationMode,
) -> IntegrationPlanResult {
    let mut items = Vec::new();
    let mut auto_ok_count = 0u32;
    let mut review_count = 0u32;
    let mut blocked_count = 0u32;

    for fc in files {
        let path = fc
            .target_path
            .clone()
            .unwrap_or_else(|| fc.path.clone());
        let (integration_status, strategy, reason) =
            classify_file(fc, target_wc_path, target_kind, mode);
        match integration_status {
            IntegrationStatus::AutoOk => auto_ok_count += 1,
            IntegrationStatus::Review => review_count += 1,
            IntegrationStatus::Blocked => blocked_count += 1,
        }
        let before_lines = to_lines(fc.before.as_deref());
        let after_lines = to_lines(fc.after.as_deref());
        let diff = integration_diff_lines(fc);
        items.push(IntegrationItemView {
            id: path.clone(),
            path,
            status: file_kind_to_status(&fc.kind),
            integration_status,
            strategy,
            reason,
            overlap_lines: find_overlap_lines(&before_lines, &after_lines),
            location_status: fc.analysis.as_ref().map(|a| a.location_status.clone()),
            merge_status: fc.analysis.as_ref().map(|a| a.merge_status.clone()),
            match_method: fc.analysis.as_ref().map(|a| a.match_method.clone()),
            confidence: fc.analysis.as_ref().map(|a| a.confidence),
            candidate_count: fc.analysis.as_ref().map(|a| a.candidate_count),
            before: before_lines,
            after: after_lines,
            diff,
        });
    }

    IntegrationPlanResult {
        mode,
        auto_ok_count,
        review_count,
        blocked_count,
        items,
    }
}

pub fn strategy_map(plan: &IntegrationPlanResult) -> HashMap<String, IntegrationStrategy> {
    plan.items
        .iter()
        .map(|i| (i.path.clone(), i.strategy))
        .collect()
}

fn classify_file(
    fc: &FileChange,
    target_wc_path: &str,
    target_kind: TargetWcKind,
    mode: MigrationMode,
) -> (IntegrationStatus, IntegrationStrategy, String) {
    if target_has_expected_content(fc) {
        return (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::Skip,
            "目标已包含期望内容".into(),
        );
    }

    match fc.kind {
        FileChangeKind::Add => classify_add(fc),
        FileChangeKind::Delete => classify_delete(fc),
        FileChangeKind::Binary => (
            IntegrationStatus::Review,
            IntegrationStrategy::WriteAfter,
            "二进制文件需确认后写入".into(),
        ),
        FileChangeKind::Modify | FileChangeKind::Rename => {
            classify_modify(fc, target_wc_path, target_kind, mode)
        }
    }
}

fn classify_add(fc: &FileChange) -> (IntegrationStatus, IntegrationStrategy, String) {
    if target_has_expected_content(fc) {
        return (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::Skip,
            "目标已包含期望内容".into(),
        );
    }
    if fc.before.is_none() {
        return (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::WriteAfter,
            "将新增文件".into(),
        );
    }
    (
        IntegrationStatus::Blocked,
        IntegrationStrategy::ManualMerge,
        "目标路径已存在文件".into(),
    )
}

fn classify_delete(fc: &FileChange) -> (IntegrationStatus, IntegrationStrategy, String) {
    if fc.before.is_none() {
        return (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::Skip,
            "目标路径不存在，跳过删除".into(),
        );
    }
    (
        IntegrationStatus::Review,
        IntegrationStrategy::WriteAfter,
        "将删除目标文件，请确认".into(),
    )
}

fn classify_modify(
    fc: &FileChange,
    target_wc_path: &str,
    target_kind: TargetWcKind,
    mode: MigrationMode,
) -> (IntegrationStatus, IntegrationStrategy, String) {
    if let Some(analysis) = &fc.analysis {
        return classify_analysis(analysis);
    }

    if fc.conflict_risk == Some(crate::model::ConflictRisk::High) {
        return (
            IntegrationStatus::Blocked,
            IntegrationStrategy::ManualMerge,
            "目标与源补丁上下文不一致，需人工合并".into(),
        );
    }
    let patch_ok = fc.patch.is_some()
        && patch_applies_cleanly(target_kind, target_wc_path, fc);
    let source_before = fc
        .patch
        .as_deref()
        .map(reconstruct_old_from_patch)
        .filter(|s| !s.is_empty());
    let overlap = has_meaningful_overlap(
        fc.before.as_deref(),
        source_before.as_deref(),
        fc.after.as_deref(),
    );
    let target_matches_source_before =
        source_before.as_ref().is_some_and(|sb| content_equal(fc.before.as_deref(), Some(sb)));

    match mode {
        MigrationMode::StrictReplay => {
            if patch_ok {
                (
                    IntegrationStatus::AutoOk,
                    IntegrationStrategy::ApplyPatch,
                    "补丁可干净应用".into(),
                )
            } else {
                (
                    IntegrationStatus::Blocked,
                    IntegrationStrategy::ManualMerge,
                    "严格模式：补丁无法干净应用".into(),
                )
            }
        }
        MigrationMode::CommitResult => {
            if overlap && !target_matches_source_before {
                (
                    IntegrationStatus::Blocked,
                    IntegrationStrategy::ManualMerge,
                    "目标有独立修改，与迁入变更区域重叠".into(),
                )
            } else {
                (
                    IntegrationStatus::Review,
                    IntegrationStrategy::WriteAfter,
                    "将以提交后完整内容写入目标".into(),
                )
            }
        }
        MigrationMode::IncrementalFirst => {
            if patch_ok {
                (
                    IntegrationStatus::AutoOk,
                    IntegrationStrategy::ApplyPatch,
                    "补丁可干净应用".into(),
                )
            } else if fc.patch.is_none()
                && !content_equal(fc.before.as_deref(), fc.after.as_deref())
            {
                (
                    IntegrationStatus::AutoOk,
                    IntegrationStrategy::WriteAfter,
                    "已合并所选提交的净变更".into(),
                )
            } else if target_matches_source_before {
                (
                    IntegrationStatus::Review,
                    IntegrationStrategy::WriteAfter,
                    "基线不一致，将写入提交后完整内容".into(),
                )
            } else if overlap {
                (
                    IntegrationStatus::Blocked,
                    IntegrationStrategy::ManualMerge,
                    "目标有独立修改，与迁入变更区域重叠".into(),
                )
            } else {
                (
                    IntegrationStatus::Review,
                    IntegrationStrategy::WriteAfter,
                    "目标内容与源基线不同，将写入提交后完整内容".into(),
                )
            }
        }
    }
}

fn classify_analysis(
    analysis: &crate::model::ChangeAnalysis,
) -> (IntegrationStatus, IntegrationStrategy, String) {
    match (&analysis.location_status, &analysis.merge_status) {
        (LocationStatus::Exact, MergeStatus::AutoApply) => (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::ApplyPatch,
            analysis.reason.clone(),
        ),
        (LocationStatus::ContextDrift, MergeStatus::AutoMerge) => (
            IntegrationStatus::Review,
            IntegrationStrategy::WriteAfter,
            analysis.reason.clone(),
        ),
        (LocationStatus::AlreadyContains, MergeStatus::Skip) => (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::Skip,
            analysis.reason.clone(),
        ),
        (_, MergeStatus::KeepTarget) => (
            IntegrationStatus::AutoOk,
            IntegrationStrategy::Skip,
            analysis.reason.clone(),
        ),
        (
            LocationStatus::MultipleCandidates
            | LocationStatus::NotFound
            | LocationStatus::SameRegionConflict
            | LocationStatus::FileMissing
            | LocationStatus::PathUnmapped,
            _,
        )
        | (_, MergeStatus::Conflict) => (
            IntegrationStatus::Blocked,
            IntegrationStrategy::ManualMerge,
            analysis.reason.clone(),
        ),
        _ => (
            IntegrationStatus::Review,
            IntegrationStrategy::WriteAfter,
            analysis.reason.clone(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{ChangeAnalysis, LocationStatus, MatchMethod, MergeStatus},
        store::models::{IntegrationStatus, IntegrationStrategy},
    };

    fn modify_fc(before: &str, after: &str, patch: &str) -> FileChange {
        FileChange {
            path: "/trunk/a.txt".into(),
            target_path: Some("a.txt".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: Some(before.into()),
            after: Some(after.into()),
            source_after: None,
            source_ref: None,
            patch: Some(patch.into()),
            conflict_risk: None,
            analysis: None,
        }
    }

    #[test]
    fn blocked_when_conflict_risk_high() {
        let fc = FileChange {
            path: "/trunk/a.html".into(),
            target_path: Some("a.html".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: Some("target\n".into()),
            after: Some("target\n".into()),
            source_after: None,
            source_ref: None,
            patch: Some("@@ -1,1 +1,1 @@\n-old\n+new\n".into()),
            conflict_risk: Some(crate::model::ConflictRisk::High),
            analysis: None,
        };
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.items[0].integration_status, IntegrationStatus::Blocked);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::ManualMerge);
    }

    #[test]
    fn skip_when_target_equals_after() {
        let fc = modify_fc("same\n", "same\n", "@@ -1 +1 @@\n same\n");
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.auto_ok_count, 1);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::Skip);
    }

    #[test]
    fn add_blocked_when_path_occupied() {
        let fc = FileChange {
            path: "/trunk/n.txt".into(),
            target_path: Some("n.txt".into()),
            kind: FileChangeKind::Add,
            old_path: None,
            before: Some("exists\n".into()),
            after: Some("new\n".into()),
            source_after: None,
            source_ref: None,
            patch: Some("@@ -0,0 +1,1 @@\n+new\n".into()),
            conflict_risk: None,
            analysis: None,
        };
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.blocked_count, 1);
        assert_eq!(plan.items[0].integration_status, IntegrationStatus::Blocked);
    }

    #[test]
    fn strict_replay_blocks_when_patch_context_mismatch() {
        let fc = modify_fc(
            "git version\n",
            "svn version\n",
            "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n",
        );
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::StrictReplay,
        );
        assert_eq!(plan.blocked_count, 1);
        assert_eq!(plan.items[0].reason, "严格模式：补丁无法干净应用");
    }

    #[test]
    fn incremental_review_when_baseline_differs() {
        let fc = modify_fc(
            "git version\n",
            "new line\nunchanged\n",
            "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n",
        );
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.review_count, 1);
        assert_eq!(plan.items[0].integration_status, IntegrationStatus::Review);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::WriteAfter);
    }

    #[test]
    fn blocked_when_both_sides_changed_same_line() {
        let fc = modify_fc(
            "header\ntarget edit\nfooter\n",
            "header\nincoming\nfooter\n",
            "@@ -1,3 +1,3 @@\n header\n-old\n+incoming\n footer\n",
        );
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.blocked_count, 1);
    }

    #[test]
    fn skip_when_target_already_has_patch_result() {
        let patch = "@@ -1,3 +1,3 @@\n # Project\n-old line\n+new line\n unchanged\n";
        let fc = modify_fc(
            "# Project\nnew line\nunchanged\n",
            "# Project\nnew line\nunchanged\n",
            patch,
        );
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.auto_ok_count, 1);
        assert_eq!(plan.blocked_count, 0);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::Skip);
    }

    #[test]
    fn skip_add_when_target_file_already_matches_patch() {
        let fc = FileChange {
            path: "/trunk/n.txt".into(),
            target_path: Some("n.txt".into()),
            kind: FileChangeKind::Add,
            old_path: None,
            before: Some("line1\nline2\n".into()),
            after: Some("line1\nline2\n".into()),
            source_after: None,
            source_ref: None,
            patch: Some("@@ -0,0 +1,2 @@\n+line1\n+line2\n".into()),
            conflict_risk: None,
            analysis: None,
        };
        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );
        assert_eq!(plan.auto_ok_count, 1);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::Skip);
    }

    fn analysis(
        location_status: LocationStatus,
        merge_status: MergeStatus,
        reason: &str,
    ) -> ChangeAnalysis {
        ChangeAnalysis {
            target_path: Some("a.txt".into()),
            location_status,
            match_method: MatchMethod::Context,
            confidence: 0.75,
            candidate_count: 1,
            merge_status,
            reason: reason.into(),
        }
    }

    #[test]
    fn analysis_exact_auto_apply_drives_auto_ok() {
        let mut fc = modify_fc(
            "alpha\nold\nomega\n",
            "alpha\nnew\nomega\n",
            "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n",
        );
        fc.analysis = Some(analysis(
            LocationStatus::Exact,
            MergeStatus::AutoApply,
            "source old block matched",
        ));

        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );

        assert_eq!(plan.auto_ok_count, 1);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::ApplyPatch);
        assert_eq!(plan.items[0].location_status, Some(LocationStatus::Exact));
    }

    #[test]
    fn analysis_context_drift_auto_merge_requires_review() {
        let mut fc = modify_fc(
            "header\nalpha\nold\nomega\nfooter\n",
            "header\nalpha\nnew\nomega\nfooter\n",
            "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n",
        );
        fc.conflict_risk = Some(crate::model::ConflictRisk::High);
        fc.analysis = Some(analysis(
            LocationStatus::ContextDrift,
            MergeStatus::AutoMerge,
            "context drift",
        ));

        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );

        assert_eq!(plan.review_count, 1);
        assert_eq!(plan.items[0].integration_status, IntegrationStatus::Review);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::WriteAfter);
        assert_eq!(plan.items[0].merge_status, Some(MergeStatus::AutoMerge));
    }

    #[test]
    fn analysis_already_contains_skips() {
        let mut fc = modify_fc(
            "alpha\nnew\nomega\n",
            "alpha\nnew\nomega\n",
            "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n",
        );
        fc.analysis = Some(analysis(
            LocationStatus::AlreadyContains,
            MergeStatus::Skip,
            "already contained",
        ));

        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );

        assert_eq!(plan.auto_ok_count, 1);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::Skip);
    }

    #[test]
    fn analysis_conflict_status_blocks() {
        let mut fc = modify_fc(
            "alpha\ntarget\nomega\n",
            "alpha\ntarget\nomega\n",
            "@@ -1,3 +1,3 @@\n alpha\n-old\n+new\n omega\n",
        );
        fc.analysis = Some(analysis(
            LocationStatus::SameRegionConflict,
            MergeStatus::Conflict,
            "same region conflict",
        ));

        let plan = build_integration_plan(
            &[fc],
            "/tmp",
            TargetWcKind::Git,
            MigrationMode::IncrementalFirst,
        );

        assert_eq!(plan.blocked_count, 1);
        assert_eq!(plan.items[0].strategy, IntegrationStrategy::ManualMerge);
    }
}
