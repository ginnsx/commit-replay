use tauri::State;

use crate::{
    error::AppError,
    preview::{build_integration_plan, build_preview_plan_parallel, strategy_map, MappingInput},
    store::{
        db::{decrypt_repo_pass, get_repo, repo_path_mappings, save_migration, DbState},
        models::{
            CommitSnapshot, IntegrationStatus, IntegrationStrategy, MigrationMode, MigrationRecord,
            RepoSnapshot, RepoType,
        },
    },
    vcs::{git_writer::GitWriter, VcsWriter},
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

fn unit_strategy(
    aggregated: IntegrationStrategy,
    _unit_is_last_for_path: bool,
) -> IntegrationStrategy {
    match aggregated {
        IntegrationStrategy::Skip => IntegrationStrategy::Skip,
        IntegrationStrategy::ManualMerge => IntegrationStrategy::ManualMerge,
        IntegrationStrategy::WriteAfter => IntegrationStrategy::WriteAfter,
        IntegrationStrategy::ApplyPatch => IntegrationStrategy::ApplyPatch,
    }
}

#[tauri::command]
pub fn execute_migration(
    state: State<DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    conflicts_resolved: u32,
    path_mappings: Option<Vec<MappingInput>>,
    migration_mode: Option<MigrationMode>,
    accepted_review: Option<Vec<String>>,
    resolved_blocked: Option<Vec<String>>,
) -> Result<MigrationResult, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let source = get_repo(&conn, &source_id)?
        .ok_or_else(|| AppError::Vcs("source not found".into()))?;
    let target = get_repo(&conn, &target_id)?
        .ok_or_else(|| AppError::Vcs("target not found".into()))?;
    if !matches!(source.repo_type, RepoType::Svn) || !matches!(target.repo_type, RepoType::Git) {
        return Err(AppError::Validation("only SVN → Git supported".into()));
    }
    let mappings: Vec<MappingInput> = if let Some(m) = path_mappings.filter(|m| !m.is_empty()) {
        m
    } else {
        repo_path_mappings(&source)
            .into_iter()
            .map(|m| MappingInput {
                from: m.from,
                to: m.to,
            })
            .collect()
    };
    let password = decrypt_repo_pass(&source)?;
    let preview = build_preview_plan_parallel(
        &source.path,
        &source_refs,
        &target.path,
        &mappings,
        source.svn_user.clone(),
        password,
    )?;

    let mode = migration_mode.unwrap_or(MigrationMode::IncrementalFirst);
    let plan = build_integration_plan(&preview.aggregated, &target.path, mode);
    let strategies = strategy_map(&plan);
    let accepted = accepted_review.unwrap_or_default();
    let resolved = resolved_blocked.unwrap_or_default();
    validate_integration_plan(&plan, &accepted, &resolved)?;

    let writer = GitWriter {
        repo_path: target.path.clone(),
    };
    let checkpoint = writer.prepare(&target.branch)?;
    let mut commits_applied = 0usize;
    for unit in &preview.units {
        let unit_strategies: std::collections::HashMap<String, IntegrationStrategy> = unit
            .files
            .iter()
            .filter_map(|fc| {
                let tp = fc.target_path.as_deref()?;
                let agg = strategies.get(tp).copied()?;
                Some((tp.to_string(), unit_strategy(agg, true)))
            })
            .collect();
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
        let _ = writer.commit(&unit.meta, "relay: {message}")?;
        commits_applied += 1;
    }

    let files: Vec<crate::store::models::FileChangeView> = preview
        .aggregated
        .iter()
        .map(|f| crate::relay::preview_file_with_diff(f))
        .collect();
    let commits: Vec<CommitSnapshot> = preview
        .units
        .iter()
        .map(|u| {
            let rev = u
                .meta
                .source_ref
                .strip_prefix("svn:")
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
    let resolved_count = conflicts_resolved.max(resolved.len() as u32);
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
        conflicts_resolved: resolved_count,
        status: "success".into(),
    };
    save_migration(&conn, record)?;

    Ok(MigrationResult {
        migration_id,
        commits_applied,
        files_changed: preview.aggregated.len(),
    })
}
