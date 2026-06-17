use std::collections::HashMap;

use crate::{
    diff::line_diff::find_overlap_lines,
    model::{FileChange, FileChangeKind},
    preview::git_wc::check_apply,
    preview::patch_apply::reconstruct_old_from_patch,
    relay::file_kind_to_status,
    store::models::{
        IntegrationItemView, IntegrationPlanResult, IntegrationStatus, IntegrationStrategy,
        MigrationMode,
    },
};

fn normalize_content(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn content_equal(a: Option<&str>, b: Option<&str>) -> bool {
    normalize_content(a.unwrap_or("")) == normalize_content(b.unwrap_or(""))
}

fn to_lines(text: Option<&str>) -> Vec<String> {
    text.unwrap_or("")
        .lines()
        .map(str::to_string)
        .collect()
}

fn patch_applies_cleanly(wc_root: &str, fc: &FileChange) -> bool {
    matches!(check_apply(wc_root, fc), crate::model::ConflictRisk::Low)
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

fn has_meaningful_overlap(
    target_before: Option<&str>,
    source_before: Option<&str>,
    source_after: Option<&str>,
) -> bool {
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

pub fn build_integration_plan(
    files: &[FileChange],
    target_wc_path: &str,
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
        let (integration_status, strategy, reason) = classify_file(fc, target_wc_path, mode);
        match integration_status {
            IntegrationStatus::AutoOk => auto_ok_count += 1,
            IntegrationStatus::Review => review_count += 1,
            IntegrationStatus::Blocked => blocked_count += 1,
        }
        items.push(IntegrationItemView {
            id: path.clone(),
            path,
            status: file_kind_to_status(&fc.kind),
            integration_status,
            strategy,
            reason,
            overlap_lines: find_overlap_lines(
                &to_lines(fc.before.as_deref()),
                &to_lines(fc.after.as_deref()),
            ),
            before: to_lines(fc.before.as_deref()),
            after: to_lines(fc.after.as_deref()),
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
    mode: MigrationMode,
) -> (IntegrationStatus, IntegrationStrategy, String) {
    if content_equal(fc.before.as_deref(), fc.after.as_deref()) {
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
            classify_modify(fc, target_wc_path, mode)
        }
    }
}

fn classify_add(fc: &FileChange) -> (IntegrationStatus, IntegrationStrategy, String) {
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
    mode: MigrationMode,
) -> (IntegrationStatus, IntegrationStrategy, String) {
    let patch_ok = patch_applies_cleanly(target_wc_path, fc);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::models::{IntegrationStatus, IntegrationStrategy};

    fn modify_fc(before: &str, after: &str, patch: &str) -> FileChange {
        FileChange {
            path: "/trunk/a.txt".into(),
            target_path: Some("a.txt".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: Some(before.into()),
            after: Some(after.into()),
            patch: Some(patch.into()),
            conflict_risk: None,
        }
    }

    #[test]
    fn skip_when_target_equals_after() {
        let fc = modify_fc("same\n", "same\n", "@@ -1 +1 @@\n same\n");
        let plan = build_integration_plan(&[fc], "/tmp", MigrationMode::IncrementalFirst);
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
            patch: Some("@@ -0,0 +1,1 @@\n+new\n".into()),
            conflict_risk: None,
        };
        let plan = build_integration_plan(&[fc], "/tmp", MigrationMode::IncrementalFirst);
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
        let plan = build_integration_plan(&[fc], "/tmp", MigrationMode::StrictReplay);
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
        let plan = build_integration_plan(&[fc], "/tmp", MigrationMode::IncrementalFirst);
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
        let plan = build_integration_plan(&[fc], "/tmp", MigrationMode::IncrementalFirst);
        assert_eq!(plan.blocked_count, 1);
    }
}
