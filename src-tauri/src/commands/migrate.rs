use tauri::State;

use crate::{
    comparison::migration_file_pairs,
    commit_message,
    error::AppError,
    preview::{
        build_integration_plan, build_preview_plan_parallel, patch_apply::resolve_target_after,
        strategy_map, MappingInput, PreviewContext,
    },
    relay::preview_file_with_diff,
    store::{
        db::{decrypt_repo_pass, get_repo, repo_path_mappings, save_migration, DbState},
        models::{
            CommitSnapshot, IntegrationStatus, IntegrationStrategy, MigrationMode, MigrationRecord,
            RepoSnapshot,
        },
    },
    vcs::{ensure_different_repos, MigrationWriter},
};

#[derive(Debug, serde::Serialize)]
pub struct MigrationResult {
    pub migration_id: String,
    pub commits_applied: usize,
    pub files_changed: usize,
}

fn validate_integration_plan(
    plan: &crate::store::models::IntegrationPlanResult,
    accepted_review: &[String],
    resolved_blocked: &[String],
) -> Result<(), AppError> {
    for item in &plan.items {
        match item.integration_status {
            IntegrationStatus::Blocked => {
                if !resolved_blocked.iter().any(|id| id == &item.id) {
                    return Err(AppError::Validation(format!(
                        "文件 {} 尚未解决：{}",
                        item.path, item.reason
                    )));
                }
            }
            IntegrationStatus::Review => {
                if !accepted_review.iter().any(|id| id == &item.id) {
                    return Err(AppError::Validation(format!(
                        "文件 {} 尚未确认：{}",
                        item.path, item.reason
                    )));
                }
            }
            IntegrationStatus::AutoOk => {}
        }
    }
    Ok(())
}

fn unit_apply_strategy(
    path: &str,
    aggregated: IntegrationStrategy,
    status: IntegrationStatus,
    resolved_blocked: &[String],
    squash_commits: bool,
    has_patch: bool,
) -> IntegrationStrategy {
    let eff = effective_strategy(path, aggregated, resolved_blocked);
    if eff == IntegrationStrategy::WriteAfter && resolved_blocked.iter().any(|id| id == path) {
        return IntegrationStrategy::Skip;
    }
    if squash_commits && eff == IntegrationStrategy::WriteAfter {
        IntegrationStrategy::Skip
    } else if !squash_commits
        && status == IntegrationStatus::AutoOk
        && eff == IntegrationStrategy::WriteAfter
        && has_patch
    {
        IntegrationStrategy::ApplyPatch
    } else {
        eff
    }
}

fn unit_has_applicable_changes(
    unit: &crate::model::PreviewUnit,
    strategies: &std::collections::HashMap<String, IntegrationStrategy>,
) -> bool {
    unit.files.iter().any(|fc| {
        let Some(target_path) = fc.target_path.as_deref() else {
            return true;
        };
        strategies
            .get(target_path)
            .copied()
            .unwrap_or(IntegrationStrategy::ApplyPatch)
            != IntegrationStrategy::Skip
    })
}

fn prepare_finalize_file(
    fc: &crate::model::FileChange,
    resolved_blocked: &[String],
    units: &[crate::model::PreviewUnit],
) -> Option<crate::model::FileChange> {
    let tp = fc.target_path.as_deref()?;
    let mut out = fc.clone();
    if resolved_blocked.iter().any(|id| id == tp) {
        let before = out.before.as_deref().unwrap_or("");
        let patches: Vec<&str> = units
            .iter()
            .flat_map(|u| u.files.iter())
            .filter(|f| f.target_path.as_deref() == Some(tp))
            .filter_map(|f| f.patch.as_deref())
            .collect();
        if let Some(after) = resolve_target_after(before, out.patch.as_deref(), &patches) {
            out.after = Some(after);
        }
    }
    if content_equal(out.before.as_deref(), out.after.as_deref()) {
        return None;
    }
    Some(out)
}

fn should_finalize_path(
    path: &str,
    strategy: IntegrationStrategy,
    resolved_blocked: &[String],
    squash_commits: bool,
    migration_mode: MigrationMode,
) -> bool {
    if effective_strategy(path, strategy, resolved_blocked) != IntegrationStrategy::WriteAfter {
        return false;
    }
    squash_commits
        || migration_mode == MigrationMode::CommitResult
        || resolved_blocked.iter().any(|id| id == path)
}

fn effective_strategy(
    path: &str,
    strategy: IntegrationStrategy,
    resolved_blocked: &[String],
) -> IntegrationStrategy {
    if strategy == IntegrationStrategy::ManualMerge && resolved_blocked.iter().any(|id| id == path)
    {
        IntegrationStrategy::WriteAfter
    } else {
        strategy
    }
}

fn content_equal(a: Option<&str>, b: Option<&str>) -> bool {
    let norm = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
    norm(a.unwrap_or("")) == norm(b.unwrap_or(""))
}

fn preview_context(
    source: &crate::store::models::RepoRecord,
    target: &crate::store::models::RepoRecord,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<PreviewContext, AppError> {
    let mappings: Vec<MappingInput> = if let Some(m) = path_mappings.filter(|m| !m.is_empty()) {
        m
    } else {
        repo_path_mappings(source)
            .into_iter()
            .map(|m| MappingInput {
                from: m.from,
                to: m.to,
            })
            .collect()
    };
    let password = decrypt_repo_pass(source)?;
    PreviewContext::from_repos(source, target, password, mappings).map_err(Into::into)
}

struct MigrationWork {
    source: crate::store::models::RepoRecord,
    target: crate::store::models::RepoRecord,
    source_refs: Vec<String>,
    conflicts_resolved: u32,
    path_mappings: Option<Vec<MappingInput>>,
    migration_mode: MigrationMode,
    accepted_review: Vec<String>,
    resolved_blocked: Vec<String>,
    squash_commits: bool,
    squash_commit_message: Option<String>,
}

struct MigrationOutput {
    result: MigrationResult,
    record: MigrationRecord,
}

fn run_migration(work: MigrationWork) -> Result<MigrationOutput, AppError> {
    let MigrationWork {
        source,
        target,
        source_refs,
        conflicts_resolved,
        path_mappings,
        migration_mode,
        accepted_review,
        resolved_blocked,
        squash_commits,
        squash_commit_message,
    } = work;

    let squash_message = if squash_commits {
        let msg = squash_commit_message
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::Validation("合并为单次提交时需填写 commit message".into()))?;
        Some(msg.to_string())
    } else {
        None
    };

    ensure_different_repos(&source, &target)?;

    let ctx = preview_context(&source, &target, path_mappings)?;
    let preview = build_preview_plan_parallel(&ctx, &source_refs)?;

    let plan = build_integration_plan(
        &preview.aggregated,
        &ctx.target_wc_path,
        ctx.target_kind,
        migration_mode,
    );
    let strategies = strategy_map(&plan);
    let statuses: std::collections::HashMap<String, IntegrationStatus> = plan
        .items
        .iter()
        .map(|i| (i.path.clone(), i.integration_status))
        .collect();
    validate_integration_plan(&plan, &accepted_review, &resolved_blocked)?;

    let target_password = decrypt_repo_pass(&target)?;
    let writer = MigrationWriter::from_repo(&target, target_password)?;
    let checkpoint = writer.prepare(&target.branch)?;
    let mut commits_applied = 0usize;
    for unit in &preview.units {
        let unit_strategies: std::collections::HashMap<String, IntegrationStrategy> = unit
            .files
            .iter()
            .filter_map(|fc| {
                let tp = fc.target_path.as_deref()?;
                let agg = strategies.get(tp).copied()?;
                let status = statuses.get(tp).copied()?;
                Some((
                    tp.to_string(),
                    unit_apply_strategy(
                        tp,
                        agg,
                        status,
                        &resolved_blocked,
                        squash_commits,
                        fc.patch.is_some(),
                    ),
                ))
            })
            .collect();
        if !unit_has_applicable_changes(unit, &unit_strategies) {
            continue;
        }
        let apply = writer.apply_changeset_with_strategies(
            &crate::model::ChangeSet {
                meta: unit.meta.clone(),
                files: unit.files.clone(),
            },
            &unit_strategies,
        )?;
        if apply.status != crate::model::ApplyStatus::Ok {
            let _ = writer.rollback(&checkpoint);
            return Err(AppError::Apply(format!(
                "failed to apply {}: {:?}",
                unit.meta.source_ref, apply.failed_paths
            )));
        }
        if !squash_commits {
            let message = commit_message::replay_message(&unit.meta);
            if let Err(err) = writer.commit_allow_empty(&unit.meta, &message) {
                let _ = writer.rollback(&checkpoint);
                return Err(err);
            }
            commits_applied += 1;
        }
    }

    let finalize_files: Vec<crate::model::FileChange> = preview
        .aggregated
        .iter()
        .filter(|fc| {
            fc.target_path.as_ref().is_some_and(|tp| {
                strategies.get(tp).is_some_and(|s| {
                    should_finalize_path(tp, *s, &resolved_blocked, squash_commits, migration_mode)
                })
            })
        })
        .filter_map(|fc| prepare_finalize_file(fc, &resolved_blocked, &preview.units))
        .collect();
    if !finalize_files.is_empty() {
        let finalize_strategies: std::collections::HashMap<String, IntegrationStrategy> =
            finalize_files
                .iter()
                .filter_map(|fc| {
                    let tp = fc.target_path.as_deref()?;
                    Some((tp.to_string(), IntegrationStrategy::WriteAfter))
                })
                .collect();
        let finalize_meta = preview
            .units
            .last()
            .map(|u| u.meta.clone())
            .unwrap_or_else(|| preview.units[0].meta.clone());
        let apply = writer.apply_changeset_with_strategies(
            &crate::model::ChangeSet {
                meta: finalize_meta,
                files: finalize_files,
            },
            &finalize_strategies,
        )?;
        if apply.status != crate::model::ApplyStatus::Ok {
            let _ = writer.rollback(&checkpoint);
            return Err(AppError::Apply(format!(
                "failed to finalize write_after: {:?}",
                apply.failed_paths
            )));
        }
        if !squash_commits {
            let message = commit_message::finalize_message();
            if let Err(err) = writer.commit_with_message(&message) {
                let _ = writer.rollback(&checkpoint);
                return Err(err);
            }
            commits_applied += 1;
        }
    }

    if squash_commits {
        let msg = squash_message.as_deref().expect("validated above");
        let metas: Vec<crate::model::ReplayUnitMeta> =
            preview.units.iter().map(|u| u.meta.clone()).collect();
        let message = commit_message::squash_message(msg, &metas);
        if let Err(err) = writer.commit_with_message(&message) {
            let _ = writer.rollback(&checkpoint);
            return Err(err);
        }
        commits_applied = 1;
    }

    let files: Vec<crate::store::models::FileChangeView> = preview
        .aggregated
        .iter()
        .map(|f| preview_file_with_diff(f))
        .collect();
    let comparison_files = migration_file_pairs(&preview.aggregated);
    let commits: Vec<CommitSnapshot> = preview
        .units
        .iter()
        .map(|u| {
            let rev = u
                .meta
                .source_ref
                .strip_prefix("svn:")
                .or_else(|| u.meta.source_ref.strip_prefix("git:"))
                .unwrap_or(&u.meta.source_ref);
            CommitSnapshot {
                id: u.meta.source_ref.clone(),
                hash: rev.chars().take(7).collect(),
                msg: u.meta.message.clone(),
                author: u.meta.author.clone(),
                date: u.meta.date.clone(),
                files: u.files.len(),
            }
        })
        .collect();

    let migration_id = format!("m{}", chrono::Utc::now().timestamp_millis());
    let resolved_count = conflicts_resolved.max(resolved_blocked.len() as u32);
    let record = MigrationRecord {
        id: migration_id.clone(),
        completed_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        source: RepoSnapshot {
            name: source.name.clone(),
            path: source.path.clone(),
            repo_type: source.repo_type.clone(),
            branch: source.branch.clone(),
        },
        target: RepoSnapshot {
            name: target.name.clone(),
            path: target.path.clone(),
            repo_type: target.repo_type.clone(),
            branch: target.branch.clone(),
        },
        commits,
        files,
        comparison_files,
        conflicts_resolved: resolved_count,
        status: "success".into(),
    };

    Ok(MigrationOutput {
        result: MigrationResult {
            migration_id: migration_id.clone(),
            commits_applied,
            files_changed: preview.aggregated.len(),
        },
        record,
    })
}

#[tauri::command]
pub async fn execute_migration(
    state: State<'_, DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    conflicts_resolved: u32,
    path_mappings: Option<Vec<MappingInput>>,
    migration_mode: Option<MigrationMode>,
    accepted_review: Option<Vec<String>>,
    resolved_blocked: Option<Vec<String>>,
    squash_commits: Option<bool>,
    squash_commit_message: Option<String>,
) -> Result<MigrationResult, AppError> {
    let work = {
        let conn = state
            .0
            .lock()
            .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
        let source =
            get_repo(&conn, &source_id)?.ok_or_else(|| AppError::Vcs("source not found".into()))?;
        let target =
            get_repo(&conn, &target_id)?.ok_or_else(|| AppError::Vcs("target not found".into()))?;
        MigrationWork {
            source,
            target,
            source_refs,
            conflicts_resolved,
            path_mappings,
            migration_mode: migration_mode.unwrap_or(MigrationMode::IncrementalFirst),
            accepted_review: accepted_review.unwrap_or_default(),
            resolved_blocked: resolved_blocked.unwrap_or_default(),
            squash_commits: squash_commits.unwrap_or(false),
            squash_commit_message,
        }
    };

    let output = tokio::task::spawn_blocking(move || run_migration(work))
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("migration task: {e}")))??;

    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let commit_refs: Vec<String> = output.record.commits.iter().map(|c| c.id.clone()).collect();
    let relayed_at = output.record.completed_at.clone();
    save_migration(&conn, output.record)?;
    crate::store::db::record_relayed_commits(&conn, &source_id, &commit_refs, &relayed_at)?;

    Ok(output.result)
}
