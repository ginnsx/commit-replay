use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use copy_diff_lib::{
    comparison::migration_file_pairs,
    commit_message,
    error::{AppError, Result as AppResult},
    model::{ApplyStatus, ChangeSet, FileChange, PreviewUnit},
    preview::{
        build_integration_plan, build_preview_plan_parallel, patch_apply::resolve_target_after,
        strategy_map, MappingInput, PreviewContext,
    },
    relay::preview_file_with_diff,
    store::models::{
        CommitSnapshot, FileChangeView, IntegrationStatus, IntegrationStrategy, MigrationMode,
        MigrationRecord, RepoRecord, RepoSnapshot, RepoType,
    },
    vcs::{svn::parse_log_xml, MigrationWriter, VcsCheckpoint},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const BASE_EXISTING: &str = "alpha\nold\nomega\n";
const MOD_EXISTING: &str = "alpha\nnew\nomega\n";
const DRIFT_EXISTING: &str = "header\nalpha\nold\nomega\nfooter\n";
const DRIFT_MOD_EXISTING: &str = "header\nalpha\nnew\nomega\nfooter\n";
const SAME_REGION_TARGET: &str = "alpha\ntarget\nomega\n";
const MULTI_CANDIDATE_EXISTING: &str = "alpha\nold\nomega\nbetween\nalpha\nold\nomega\n";
const BASE_CHAIN: &str = "one\nbase\nthree\n";
const STEP1_CHAIN: &str = "one\nstep1\nthree\n";
const STEP2_CHAIN: &str = "one\nstep2\nthree\n";
const MOVE_ME: &str = "move\nme\n";
const NEW_FILE: &str = "new\nfile\ncontent\n";
const DELETE_ME: &str = "delete\nme\n";
const TARGET_ONLY: &str = "target\nonly\n";
const README: &str = "relay regression target\n";
const CRLF_FILE: &str = "first\r\nsecond\r\n";
const NO_FINAL_NEWLINE: &str = "no final newline";
const EMPTY_FILE: &str = "";
const CHINESE_FILE: &str = "中文内容\n第二行\n";
const MODULE_FILE: &str = "module mapped\n";
const CASE_FILE: &str = "case\n";
const BINARY_FILE: &[u8] = b"relay\0binary\nafter\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suite {
    Smoke,
    ProductionSafety,
    Release,
    UiSmoke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VcsKind {
    Git,
    Svn,
}

#[derive(Debug, Clone)]
struct CaseSpec {
    id: &'static str,
    source: VcsKind,
    target: VcsKind,
    commits: Vec<&'static str>,
    mappings: Vec<MappingInput>,
    target_setup: TargetSetup,
    migration_mode: MigrationMode,
    squash_commits: bool,
    squash_message: Option<&'static str>,
    expect: Expectation,
    accept_review: bool,
    resolved_blocked: Vec<&'static str>,
    checks_history: bool,
}

impl CaseSpec {
    fn with_accept_review(mut self) -> Self {
        self.accept_review = true;
        self
    }
}

#[derive(Debug, Clone, Copy)]
enum TargetSetup {
    Clean,
    Dirty,
    OccupiedNewFile,
    ExistingDirectoryAtNewFile,
    AlreadyHasModify,
    DriftedModify,
    SameRegionModify,
    MultiCandidateModify,
}

#[derive(Debug, Clone)]
enum ExpectedContent {
    Text(&'static str),
    Bytes(&'static [u8]),
}

#[derive(Debug, Clone)]
enum Expectation {
    Success {
        changed: BTreeMap<&'static str, ExpectedContent>,
        deleted: Vec<&'static str>,
        absent: Vec<&'static str>,
        unchanged: Vec<&'static str>,
        commit_count: usize,
    },
    Failure {
        phase: &'static str,
        contains: &'static str,
        target_unchanged: bool,
    },
}

#[derive(Debug, Serialize)]
struct CaseResult {
    case_id: String,
    status: String,
    message: String,
    case_dir: String,
}

#[derive(Debug, Serialize)]
struct ErrorRecord {
    case_id: String,
    phase: String,
    error_type: String,
    message: String,
    expected: serde_json::Value,
    actual: serde_json::Value,
    artifacts: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct Summary {
    run_id: String,
    suite: String,
    passed: usize,
    failed: usize,
    skipped: usize,
    cases: Vec<CaseResult>,
}

#[derive(Debug, Clone)]
struct PhaseError {
    phase: &'static str,
    error_type: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct Manifest {
    files: BTreeMap<String, String>,
}

#[derive(Debug)]
struct RepoFixture {
    wc_path: PathBuf,
    refs: BTreeMap<String, String>,
}

#[derive(Debug)]
struct MigrationOutput {
    commits_applied: usize,
    created_branch: Option<String>,
    record: MigrationRecord,
}

fn main() {
    if env::var("COPY_DIFF_COMMAND_TIMEOUT_SECS").is_err() {
        env::set_var("COPY_DIFF_COMMAND_TIMEOUT_SECS", "20");
    }
    let cfg = Config::from_args();
    let result = run_suite(&cfg);
    match result {
        Ok(summary) => {
            println!(
                "regression {}: {} passed, {} failed, {} skipped",
                summary.run_id, summary.passed, summary.failed, summary.skipped
            );
            if summary.failed > 0 {
                for case in summary.cases.iter().filter(|case| case.status == "failed") {
                    eprintln!("FAILED {}: {}", case.case_id, case.message);
                }
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("regression runner failed: {e}");
            std::process::exit(2);
        }
    }
}

struct Config {
    suite: Suite,
    out_dir: PathBuf,
    keep_passed: bool,
    case_filter: Option<String>,
}

impl Config {
    fn from_args() -> Self {
        let mut suite = Suite::ProductionSafety;
        let mut out_dir = PathBuf::from("../artifacts/regression");
        let mut keep_passed = false;
        let mut case_filter = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--suite" => {
                    let value = args.next().unwrap_or_else(|| "production-safety".into());
                    suite = match value.as_str() {
                        "smoke" => Suite::Smoke,
                        "production-safety" | "safety" => Suite::ProductionSafety,
                        "release" => Suite::Release,
                        "ui-smoke" | "ui" => Suite::UiSmoke,
                        other => {
                            eprintln!("unknown suite {other}, using production-safety");
                            Suite::ProductionSafety
                        }
                    };
                }
                "--out-dir" => {
                    out_dir = PathBuf::from(
                        args.next()
                            .unwrap_or_else(|| "../artifacts/regression".into()),
                    );
                }
                "--keep-passed" => keep_passed = true,
                "--case" => case_filter = args.next(),
                _ => {}
            }
        }
        Self {
            suite,
            out_dir,
            keep_passed,
            case_filter,
        }
    }
}

fn run_suite(cfg: &Config) -> AppResult<Summary> {
    let run_id = new_run_id();
    let run_dir = cfg.out_dir.join(&run_id);
    fs::create_dir_all(run_dir.join("cases"))?;

    let mut summary = Summary {
        run_id: run_id.clone(),
        suite: suite_name(cfg.suite).into(),
        passed: 0,
        failed: 0,
        skipped: 0,
        cases: Vec::new(),
    };

    if cfg.suite == Suite::UiSmoke {
        let result = run_ui_smoke(&run_dir)?;
        summary.skipped += usize::from(result.status == "skipped");
        summary.failed += usize::from(result.status == "failed");
        summary.passed += usize::from(result.status == "passed");
        summary.cases.push(result);
    } else {
        for case in cases_for_suite(cfg.suite)
            .into_iter()
            .filter(|case| cfg.case_filter.as_ref().is_none_or(|id| id == case.id))
        {
            let case_result = run_case(&run_dir, &case);
            match case_result {
                Ok(result) => {
                    if result.status == "passed" {
                        summary.passed += 1;
                        if !cfg.keep_passed {
                            let _ = fs::remove_dir_all(&result.case_dir);
                        }
                    } else if result.status == "skipped" {
                        summary.skipped += 1;
                    } else {
                        summary.failed += 1;
                    }
                    summary.cases.push(result);
                }
                Err(err) => {
                    summary.failed += 1;
                    summary.cases.push(CaseResult {
                        case_id: case.id.into(),
                        status: "failed".into(),
                        message: err.to_string(),
                        case_dir: run_dir
                            .join("cases")
                            .join(case.id)
                            .to_string_lossy()
                            .into_owned(),
                    });
                }
            }
        }
    }

    write_json(&run_dir.join("summary.json"), &summary)?;
    write_summary_md(&run_dir.join("summary.md"), &summary)?;
    Ok(summary)
}

fn run_ui_smoke(run_dir: &Path) -> AppResult<CaseResult> {
    let case_dir = run_dir.join("cases").join("UI01-smoke");
    fs::create_dir_all(&case_dir)?;
    let result = CaseResult {
        case_id: "UI01-smoke".into(),
        status: "skipped".into(),
        message: "ui smoke requires a configured tauri-driver/WebDriver session".into(),
        case_dir: case_dir.to_string_lossy().into_owned(),
    };
    write_json(&case_dir.join("result.json"), &result)?;
    Ok(result)
}

fn run_case(run_dir: &Path, case: &CaseSpec) -> AppResult<CaseResult> {
    if case.source == VcsKind::Svn || case.target == VcsKind::Svn {
        if !command_available("svn") || !command_available("svnadmin") {
            return skipped_case(run_dir, case, "svn and svnadmin are required");
        }
    }

    let case_dir = run_dir.join("cases").join(case.id);
    fs::create_dir_all(&case_dir)?;
    write_json(&case_dir.join("case.json"), &case_to_json(case))?;

    let work = case_dir.join("work");
    fs::create_dir_all(&work)?;
    let needed_refs = case.commits.iter().copied().collect::<HashSet<_>>();
    let source = create_source_repo(
        &work.join(format!("source-{}", case.source.name())),
        case.source,
        &needed_refs,
    )?;
    let target_root = work.join(format!("target-{}", case.target.name()));
    let baseline = create_target_repo(&target_root, case.target)?;
    let target_path = target_working_copy(&target_root, case.target);
    apply_target_setup(&target_path, case.target, case.target_setup)?;
    let baseline = target_head(&target_path, case.target).unwrap_or(baseline);

    write_json(&case_dir.join("refs.json"), &source.refs)?;
    fs::write(case_dir.join("target_baseline_ref.txt"), &baseline)?;
    let before = write_manifest(&target_path, &case_dir.join("before"))?;

    let source_refs = case
        .commits
        .iter()
        .map(|name| {
            source
                .refs
                .get(*name)
                .cloned()
                .ok_or_else(|| AppError::Other(anyhow::anyhow!("missing ref {name}")))
        })
        .collect::<AppResult<Vec<_>>>()?;

    let migration = relay_migrate(&source.wc_path, &target_path, source_refs, case);
    let after = write_manifest(&target_path, &case_dir.join("after"))?;
    write_target_diff(
        &target_path,
        case.target,
        &baseline,
        &case_dir.join("after").join("target.diff"),
    );

    let result = match (&case.expect, migration) {
        (Expectation::Success { .. }, Ok(output)) => {
            verify_success(case, &target_path, &baseline, &before, &after, &output)
        }
        (Expectation::Success { .. }, Err(err)) => Err(err),
        (Expectation::Failure { phase, .. }, Ok(_)) => Err(PhaseError {
            phase,
            error_type: "unexpected_success",
            message: "migration succeeded but failure was expected".into(),
        }),
        (
            Expectation::Failure {
                phase,
                contains,
                target_unchanged,
            },
            Err(err),
        ) => {
            if err.phase != *phase {
                Err(PhaseError {
                    phase,
                    error_type: "wrong_failure_phase",
                    message: format!("expected phase {phase}, got {}", err.phase),
                })
            } else if !err
                .message
                .to_lowercase()
                .contains(&contains.to_lowercase())
            {
                Err(PhaseError {
                    phase,
                    error_type: "unexpected_failure",
                    message: format!(
                        "expected error containing {contains:?}, got {:?}",
                        err.message
                    ),
                })
            } else if *target_unchanged {
                verify_manifest_equal(&before, &after, case.id)
                    .and_then(|()| verify_failed_git_branch_state(case, &target_path, &baseline))
            } else {
                Ok(())
            }
        }
    };

    match result {
        Ok(()) => {
            let result = CaseResult {
                case_id: case.id.into(),
                status: "passed".into(),
                message: "passed".into(),
                case_dir: case_dir.to_string_lossy().into_owned(),
            };
            write_json(&case_dir.join("result.json"), &result)?;
            Ok(result)
        }
        Err(err) => {
            let record = error_record(case.id, &err, &case_dir);
            write_json(&case_dir.join("error.json"), &record)?;
            Ok(CaseResult {
                case_id: case.id.into(),
                status: "failed".into(),
                message: err.message,
                case_dir: case_dir.to_string_lossy().into_owned(),
            })
        }
    }
}

fn skipped_case(run_dir: &Path, case: &CaseSpec, reason: &str) -> AppResult<CaseResult> {
    let case_dir = run_dir.join("cases").join(case.id);
    fs::create_dir_all(&case_dir)?;
    write_json(&case_dir.join("case.json"), &case_to_json(case))?;
    let result = CaseResult {
        case_id: case.id.into(),
        status: "skipped".into(),
        message: reason.into(),
        case_dir: case_dir.to_string_lossy().into_owned(),
    };
    write_json(&case_dir.join("result.json"), &result)?;
    Ok(result)
}

fn relay_migrate(
    source_path: &Path,
    target_path: &Path,
    source_refs: Vec<String>,
    case: &CaseSpec,
) -> std::result::Result<MigrationOutput, PhaseError> {
    if case.squash_commits
        && case
            .squash_message
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .is_none()
    {
        return Err(PhaseError {
            phase: "execute",
            error_type: "validation",
            message: "squash commit message is required".into(),
        });
    }

    let source = repo_record("source", source_path, case.source);
    let target = repo_record("target", target_path, case.target);
    let ctx = PreviewContext::from_repos(&source, &target, None, case.mappings.clone())
        .map_err(|e| phase("preview", "unexpected_failure", e))?;
    let preview = build_preview_plan_parallel(&ctx, &source_refs)
        .map_err(|e| phase("preview", "unexpected_failure", e))?;
    let plan = build_integration_plan(
        &preview.aggregated,
        &target.path,
        ctx.target_kind,
        case.migration_mode,
    );
    let strategies = strategy_map(&plan);
    let statuses = plan
        .items
        .iter()
        .map(|i| (i.path.clone(), i.integration_status))
        .collect::<HashMap<_, _>>();
    let accepted_review = if case.accept_review {
        plan.items
            .iter()
            .filter(|i| i.integration_status == IntegrationStatus::Review)
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let resolved_blocked = case
        .resolved_blocked
        .iter()
        .map(|p| (*p).to_string())
        .collect::<Vec<_>>();
    validate_plan(&plan, &accepted_review, &resolved_blocked)
        .map_err(|e| phase("execute", "unexpected_failure", e))?;

    let writer = MigrationWriter::from_repo(&target, None)
        .map_err(|e| phase("execute", "unexpected_failure", e))?;
    let checkpoint = writer
        .prepare(&target.branch)
        .map_err(|e| phase("execute", "unexpected_failure", e))?;
    let mut created_branch = None;
    let execution = (|| -> AppResult<usize> {
        let mut branch_started = false;
        let mut commits_applied = 0usize;
        for unit in &preview.units {
            let unit_strategies = unit
                .files
                .iter()
                .filter_map(|fc| {
                    let target_path = fc.target_path.as_deref()?;
                    let strategy = strategies.get(target_path).copied()?;
                    let status = statuses.get(target_path).copied()?;
                    Some((
                        target_path.to_string(),
                        unit_apply_strategy(
                            target_path,
                            strategy,
                            status,
                            &resolved_blocked,
                            case.squash_commits,
                            fc.patch.is_some(),
                        ),
                    ))
                })
                .collect::<HashMap<_, _>>();
            if !unit_has_applicable_changes(unit, &unit_strategies) {
                continue;
            }
            if !branch_started {
                created_branch = writer.create_migration_branch()?;
                branch_started = true;
            }
            let apply = writer.apply_changeset_with_strategies(
                &ChangeSet {
                    meta: unit.meta.clone(),
                    files: unit.files.clone(),
                },
                &unit_strategies,
            )?;
            if apply.status != ApplyStatus::Ok {
                return Err(AppError::Apply(format!(
                    "apply failed: {:?}",
                    apply.failed_paths
                )));
            }
            if !case.squash_commits {
                let message = commit_message::replay_message(&unit.meta);
                writer.commit_allow_empty(&unit.meta, &message)?;
                commits_applied += 1;
            }
        }

        let finalize_files = preview
            .aggregated
            .iter()
            .filter(|fc| {
                fc.target_path.as_ref().is_some_and(|tp| {
                    strategies.get(tp).is_some_and(|strategy| {
                        should_finalize_path(
                            tp,
                            *strategy,
                            &resolved_blocked,
                            case.squash_commits,
                            case.migration_mode,
                        )
                    })
                })
            })
            .filter_map(|fc| prepare_finalize_file(fc, &resolved_blocked, &preview.units))
            .collect::<Vec<_>>();
        if !finalize_files.is_empty() {
            if !branch_started {
                created_branch = writer.create_migration_branch()?;
                branch_started = true;
            }
            let finalize_strategies = finalize_files
                .iter()
                .filter_map(|fc| {
                    let target_path = fc.target_path.as_deref()?;
                    Some((target_path.to_string(), IntegrationStrategy::WriteAfter))
                })
                .collect::<HashMap<_, _>>();
            let finalize_meta = preview
                .units
                .last()
                .map(|u| u.meta.clone())
                .unwrap_or_else(|| preview.units[0].meta.clone());
            let apply = writer.apply_changeset_with_strategies(
                &ChangeSet {
                    meta: finalize_meta,
                    files: finalize_files,
                },
                &finalize_strategies,
            )?;
            if apply.status != ApplyStatus::Ok {
                return Err(AppError::Apply(format!(
                    "failed to finalize write_after: {:?}",
                    apply.failed_paths
                )));
            }
            if !case.squash_commits {
                let message = commit_message::finalize_message();
                writer.commit_with_message(&message)?;
                commits_applied += 1;
            }
        }

        if case.squash_commits && branch_started {
            let msg = case.squash_message.unwrap_or("relay squash");
            let metas = preview
                .units
                .iter()
                .map(|unit| unit.meta.clone())
                .collect::<Vec<_>>();
            let message = commit_message::squash_message(msg, &metas);
            writer.commit_with_message(&message)?;
            commits_applied = 1;
        }

        Ok(commits_applied)
    })();
    let commits_applied = match execution {
        Ok(commits_applied) => commits_applied,
        Err(error) => {
            return Err(execution_error_after_rollback(
                &writer,
                &checkpoint,
                created_branch.as_deref(),
                error,
            ));
        }
    };

    let target_branch = created_branch
        .clone()
        .unwrap_or_else(|| target.branch.clone());
    let record = migration_record(
        &source,
        &target,
        &target_branch,
        &preview.units,
        &preview.aggregated,
    );
    Ok(MigrationOutput {
        commits_applied,
        created_branch,
        record,
    })
}

fn execution_error_after_rollback(
    writer: &MigrationWriter,
    checkpoint: &VcsCheckpoint,
    created_branch: Option<&str>,
    error: AppError,
) -> PhaseError {
    let message = match writer.rollback(checkpoint, created_branch) {
        Ok(()) => error.to_string(),
        Err(rollback_error) => {
            format!("migration failed: {error}; rollback failed: {rollback_error}")
        }
    };
    PhaseError {
        phase: "execute",
        error_type: "unexpected_failure",
        message,
    }
}

fn unit_apply_strategy(
    path: &str,
    strategy: IntegrationStrategy,
    status: IntegrationStatus,
    resolved_blocked: &[String],
    squash_commits: bool,
    has_patch: bool,
) -> IntegrationStrategy {
    let effective = if strategy == IntegrationStrategy::ManualMerge
        && resolved_blocked.iter().any(|id| id == path)
    {
        IntegrationStrategy::WriteAfter
    } else {
        strategy
    };
    if effective == IntegrationStrategy::WriteAfter && resolved_blocked.iter().any(|id| id == path)
    {
        return IntegrationStrategy::Skip;
    }
    if squash_commits && effective == IntegrationStrategy::WriteAfter {
        IntegrationStrategy::Skip
    } else if !squash_commits
        && status == IntegrationStatus::AutoOk
        && effective == IntegrationStrategy::WriteAfter
        && has_patch
    {
        IntegrationStrategy::ApplyPatch
    } else {
        effective
    }
}

fn unit_has_applicable_changes(
    unit: &PreviewUnit,
    strategies: &HashMap<String, IntegrationStrategy>,
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

fn should_finalize_path(
    path: &str,
    strategy: IntegrationStrategy,
    resolved_blocked: &[String],
    squash_commits: bool,
    migration_mode: MigrationMode,
) -> bool {
    let effective = if strategy == IntegrationStrategy::ManualMerge
        && resolved_blocked.iter().any(|id| id == path)
    {
        IntegrationStrategy::WriteAfter
    } else {
        strategy
    };
    effective == IntegrationStrategy::WriteAfter
        && (squash_commits
            || migration_mode == MigrationMode::CommitResult
            || resolved_blocked.iter().any(|id| id == path))
}

fn prepare_finalize_file(
    fc: &FileChange,
    resolved_blocked: &[String],
    units: &[copy_diff_lib::model::PreviewUnit],
) -> Option<FileChange> {
    let target_path = fc.target_path.as_deref()?;
    let mut out = fc.clone();
    if resolved_blocked.iter().any(|id| id == target_path) {
        let before = out.before.as_deref().unwrap_or("");
        let patches = units
            .iter()
            .flat_map(|unit| unit.files.iter())
            .filter(|file| file.target_path.as_deref() == Some(target_path))
            .filter_map(|file| file.patch.as_deref())
            .collect::<Vec<_>>();
        if let Some(after) = resolve_target_after(before, out.patch.as_deref(), &patches) {
            out.after = Some(after);
        }
    }
    if content_equal(out.before.as_deref(), out.after.as_deref()) {
        return None;
    }
    Some(out)
}

fn validate_plan(
    plan: &copy_diff_lib::store::models::IntegrationPlanResult,
    accepted_review: &[String],
    resolved_blocked: &[String],
) -> AppResult<()> {
    for item in &plan.items {
        match item.integration_status {
            IntegrationStatus::Blocked => {
                if !resolved_blocked.iter().any(|id| id == &item.id) {
                    return Err(AppError::Validation(format!(
                        "blocked item not resolved: {}",
                        item.path
                    )));
                }
            }
            IntegrationStatus::Review => {
                if !accepted_review.iter().any(|id| id == &item.id) {
                    return Err(AppError::Validation(format!(
                        "review item not accepted: {}",
                        item.path
                    )));
                }
            }
            IntegrationStatus::AutoOk => {}
        }
    }
    Ok(())
}

fn verify_success(
    case: &CaseSpec,
    target: &Path,
    baseline: &str,
    before: &Manifest,
    after: &Manifest,
    output: &MigrationOutput,
) -> std::result::Result<(), PhaseError> {
    let Expectation::Success {
        changed,
        deleted,
        absent,
        unchanged,
        commit_count,
    } = &case.expect
    else {
        return Ok(());
    };

    let actual_delta = manifest_delta(before, after);
    let expected_delta = expected_delta(changed, deleted);
    if actual_delta != expected_delta {
        return Err(PhaseError {
            phase: "verify",
            error_type: "unexpected_file",
            message: format!(
                "changed file set mismatch, expected {:?}, got {:?}",
                expected_delta, actual_delta
            ),
        });
    }

    for (path, expected_content) in changed {
        let expected_hash = expected_content.sha256();
        let actual = after.files.get(*path).ok_or_else(|| PhaseError {
            phase: "verify",
            error_type: "missing_file",
            message: format!("missing changed file {path}"),
        })?;
        if actual != &expected_hash {
            return Err(PhaseError {
                phase: "verify",
                error_type: "content_mismatch",
                message: format!("{path} hash mismatch"),
            });
        }
    }

    for path in deleted.iter().chain(absent.iter()) {
        if after.files.contains_key(*path) {
            return Err(PhaseError {
                phase: "verify",
                error_type: "unexpected_file",
                message: format!("{path} should be absent"),
            });
        }
    }

    for path in unchanged {
        if before.files.get(*path) != after.files.get(*path) {
            return Err(PhaseError {
                phase: "verify",
                error_type: "content_mismatch",
                message: format!("{path} should be unchanged"),
            });
        }
    }

    let status =
        target_status(target, case.target).map_err(|e| phase("verify", "dirty_worktree", e))?;
    if !status.trim().is_empty() {
        return Err(PhaseError {
            phase: "verify",
            error_type: "dirty_worktree",
            message: status,
        });
    }

    let count = target_commit_count(target, case.target, baseline)
        .map_err(|e| phase("verify", "commit_count_mismatch", e))?;
    if count != *commit_count {
        return Err(PhaseError {
            phase: "verify",
            error_type: "commit_count_mismatch",
            message: format!("expected {commit_count} commits, got {count}"),
        });
    }
    if output.commits_applied != *commit_count {
        return Err(PhaseError {
            phase: "verify",
            error_type: "commit_count_mismatch",
            message: format!(
                "migration reported {} commits, expected {commit_count}",
                output.commits_applied
            ),
        });
    }
    verify_successful_git_branch_state(case, target, baseline, output, *commit_count)?;
    verify_commit_messages(case, target, baseline, output, *commit_count)?;
    if case.checks_history {
        verify_history_record(case, output)?;
    }
    Ok(())
}

fn verify_commit_messages(
    case: &CaseSpec,
    target: &Path,
    baseline: &str,
    output: &MigrationOutput,
    commit_count: usize,
) -> std::result::Result<(), PhaseError> {
    if commit_count == 0 {
        return Ok(());
    }
    let messages = target_commit_messages(target, case.target, baseline)
        .map_err(|e| phase("verify", "commit_message_mismatch", e))?;
    if messages.len() != commit_count {
        return Err(PhaseError {
            phase: "verify",
            error_type: "commit_message_mismatch",
            message: format!(
                "expected {commit_count} commit messages, got {}",
                messages.len()
            ),
        });
    }
    let marker = commit_message::relay_marker();
    for message in &messages {
        if !message.contains(&marker) {
            return Err(PhaseError {
                phase: "verify",
                error_type: "commit_message_mismatch",
                message: format!("commit message missing Relay marker {marker:?}: {message:?}"),
            });
        }
    }
    if case.squash_commits {
        let message = messages.first().ok_or_else(|| PhaseError {
            phase: "verify",
            error_type: "commit_message_mismatch",
            message: "missing squash commit message".into(),
        })?;
        if !message.contains("原提交:") {
            return Err(PhaseError {
                phase: "verify",
                error_type: "commit_message_mismatch",
                message: "squash commit message missing original commit list".into(),
            });
        }
        for commit in &output.record.commits {
            let expected = format!(
                "- {} {}",
                short_record_ref(commit),
                commit_title(&commit.msg)
            );
            if !message.contains(&expected) {
                return Err(PhaseError {
                    phase: "verify",
                    error_type: "commit_message_mismatch",
                    message: format!("squash commit message missing {expected:?}"),
                });
            }
        }
    }
    Ok(())
}

fn short_record_ref(commit: &CommitSnapshot) -> String {
    if commit.id.starts_with("git:") {
        format!("git:{}", commit.hash)
    } else if commit.id.starts_with("svn:") {
        format!("svn:{}", commit.hash)
    } else {
        commit.id.clone()
    }
}

fn commit_title(message: &str) -> &str {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("无标题提交")
}

fn verify_history_record(
    case: &CaseSpec,
    output: &MigrationOutput,
) -> std::result::Result<(), PhaseError> {
    if output.record.status != "success" {
        return Err(PhaseError {
            phase: "verify",
            error_type: "history_mismatch",
            message: format!("expected success record, got {}", output.record.status),
        });
    }
    if output.record.source.repo_type.name() != case.source.name()
        || output.record.target.repo_type.name() != case.target.name()
    {
        return Err(PhaseError {
            phase: "verify",
            error_type: "history_mismatch",
            message: "record source/target repo type mismatch".into(),
        });
    }
    if output.record.commits.len() != case.commits.len() {
        return Err(PhaseError {
            phase: "verify",
            error_type: "history_mismatch",
            message: format!(
                "expected {} commits in record, got {}",
                case.commits.len(),
                output.record.commits.len()
            ),
        });
    }
    Ok(())
}

fn verify_manifest_equal(
    before: &Manifest,
    after: &Manifest,
    case_id: &str,
) -> std::result::Result<(), PhaseError> {
    if before.files == after.files {
        Ok(())
    } else {
        Err(PhaseError {
            phase: "verify",
            error_type: "rollback_failed",
            message: format!("{case_id}: target changed after expected failure"),
        })
    }
}

fn verify_successful_git_branch_state(
    case: &CaseSpec,
    target: &Path,
    baseline: &str,
    output: &MigrationOutput,
    commit_count: usize,
) -> std::result::Result<(), PhaseError> {
    if case.target != VcsKind::Git {
        return Ok(());
    }

    let current = git(target, &["branch", "--show-current"])
        .map_err(|e| phase("verify", "branch_mismatch", e))?
        .trim()
        .to_string();
    let relay_branches = git(
        target,
        &["branch", "--list", "relay/*", "--format=%(refname:short)"],
    )
    .map_err(|e| phase("verify", "branch_mismatch", e))?;
    let base_branch = case.target.branch();
    let base_head = git(target, &["rev-parse", base_branch])
        .map_err(|e| phase("verify", "branch_mismatch", e))?
        .trim()
        .to_string();

    if base_head != baseline {
        return Err(PhaseError {
            phase: "verify",
            error_type: "branch_mismatch",
            message: format!("base branch {base_branch} moved from {baseline} to {base_head}"),
        });
    }

    if commit_count == 0 {
        if output.created_branch.is_some()
            || current != base_branch
            || !relay_branches.trim().is_empty()
            || output.record.target.branch != base_branch
        {
            return Err(PhaseError {
                phase: "verify",
                error_type: "branch_mismatch",
                message: format!(
                    "no-op migration created a branch: current={current:?}, created={:?}, relay={relay_branches:?}, record={:?}",
                    output.created_branch, output.record.target.branch
                ),
            });
        }
        return Ok(());
    }

    let created_branch = output.created_branch.as_deref().ok_or_else(|| PhaseError {
        phase: "verify",
        error_type: "branch_mismatch",
        message: "Git migration did not report a created branch".into(),
    })?;
    if !created_branch.starts_with("relay/")
        || current != created_branch
        || !relay_branches
            .lines()
            .any(|branch| branch == created_branch)
        || output.record.target.branch != created_branch
    {
        return Err(PhaseError {
            phase: "verify",
            error_type: "branch_mismatch",
            message: format!(
                "created branch mismatch: current={current:?}, created={created_branch:?}, relay={relay_branches:?}, record={:?}",
                output.record.target.branch
            ),
        });
    }
    Ok(())
}

fn verify_failed_git_branch_state(
    case: &CaseSpec,
    target: &Path,
    baseline: &str,
) -> std::result::Result<(), PhaseError> {
    if case.target != VcsKind::Git {
        return Ok(());
    }

    let base_branch = case.target.branch();
    let current = git(target, &["branch", "--show-current"])
        .map_err(|e| phase("verify", "rollback_failed", e))?
        .trim()
        .to_string();
    let base_head = git(target, &["rev-parse", base_branch])
        .map_err(|e| phase("verify", "rollback_failed", e))?
        .trim()
        .to_string();
    let relay_branches = git(
        target,
        &["branch", "--list", "relay/*", "--format=%(refname:short)"],
    )
    .map_err(|e| phase("verify", "rollback_failed", e))?;
    if current == base_branch && base_head == baseline && relay_branches.trim().is_empty() {
        Ok(())
    } else {
        Err(PhaseError {
            phase: "verify",
            error_type: "rollback_failed",
            message: format!(
                "failed migration left branch state behind: current={current:?}, base_head={base_head:?}, baseline={baseline:?}, relay={relay_branches:?}"
            ),
        })
    }
}

fn create_source_repo(
    path: &Path,
    vcs: VcsKind,
    needed_refs: &HashSet<&'static str>,
) -> AppResult<RepoFixture> {
    match vcs {
        VcsKind::Git => create_git_source_repo(path, needed_refs),
        VcsKind::Svn => create_svn_source_repo(path, needed_refs),
    }
}

fn create_target_repo(path: &Path, vcs: VcsKind) -> AppResult<String> {
    match vcs {
        VcsKind::Git => create_git_target_repo(path),
        VcsKind::Svn => create_svn_target_repo(path),
    }
}

fn target_working_copy(root: &Path, vcs: VcsKind) -> PathBuf {
    match vcs {
        VcsKind::Git => root.to_path_buf(),
        VcsKind::Svn => root.join("wc"),
    }
}

fn create_git_source_repo(
    path: &Path,
    needed_refs: &HashSet<&'static str>,
) -> AppResult<RepoFixture> {
    init_git(path)?;
    write_git_baseline(path)?;
    commit(path, "baseline")?;
    let refs = write_source_commits(path, VcsKind::Git, needed_refs)?;
    Ok(RepoFixture {
        wc_path: path.to_path_buf(),
        refs,
    })
}

fn create_git_target_repo(path: &Path) -> AppResult<String> {
    init_git(path)?;
    write_git_baseline(path)?;
    commit(path, "baseline")?;
    Ok(git(path, &["rev-parse", "HEAD"])?.trim().to_string())
}

fn create_svn_source_repo(
    path: &Path,
    needed_refs: &HashSet<&'static str>,
) -> AppResult<RepoFixture> {
    fs::create_dir_all(path)?;
    let repo = path.join("repo");
    let wc = path.join("wc");
    command_status(None, "svnadmin", &["create", path_str(&repo)])?;
    command_status(None, "svn", &["checkout", &file_url(&repo), path_str(&wc)])?;
    write_svn_baseline(&wc)?;
    svn_add(&wc, "trunk")?;
    svn(&wc, &["commit", "-m", "baseline"])?;
    svn(&wc, &["update"])?;
    let refs = write_source_commits(&wc, VcsKind::Svn, needed_refs)?;
    Ok(RepoFixture { wc_path: wc, refs })
}

fn create_svn_target_repo(path: &Path) -> AppResult<String> {
    fs::create_dir_all(path)?;
    let repo = path.join("repo");
    let wc = path.join("wc");
    command_status(None, "svnadmin", &["create", path_str(&repo)])?;
    command_status(None, "svn", &["checkout", &file_url(&repo), path_str(&wc)])?;
    write_svn_baseline(&wc)?;
    svn_add(&wc, "trunk")?;
    svn(&wc, &["commit", "-m", "baseline"])?;
    svn(&wc, &["update"])?;
    svn(&wc, &["info", "--show-item", "revision"]).map(|s| s.trim().to_string())
}

fn write_source_commits(
    path: &Path,
    vcs: VcsKind,
    needed_refs: &HashSet<&'static str>,
) -> AppResult<BTreeMap<String, String>> {
    let mut refs = BTreeMap::new();
    if needed_refs.is_empty() {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/new_file.txt"), NEW_FILE)?;
    refs.insert(
        "C01-add-text".into(),
        commit_ref(path, vcs, "C01-add-text")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/existing.txt"), MOD_EXISTING)?;
    refs.insert(
        "C02-modify-text".into(),
        commit_ref(path, vcs, "C02-modify-text")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    remove_tracked_file(path, vcs, "src/delete_me.txt")?;
    refs.insert(
        "C03-delete-text".into(),
        commit_ref(path, vcs, "C03-delete-text")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    move_tracked_file(path, vcs, "src/move_me.txt", "moved/move_me.txt")?;
    refs.insert(
        "C04-rename-text".into(),
        commit_ref(path, vcs, "C04-rename-text")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_bytes(path, vcs.source_rel("assets/blob.bin"), BINARY_FILE)?;
    refs.insert(
        "C05-binary-add".into(),
        commit_ref(path, vcs, "C05-binary-add")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/crlf.txt"), CRLF_FILE)?;
    refs.insert(
        "C06-crlf-add".into(),
        commit_ref(path, vcs, "C06-crlf-add")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/chain.txt"), STEP1_CHAIN)?;
    refs.insert(
        "C07-same-file-step1".into(),
        commit_ref(path, vcs, "C07-same-file-step1")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/chain.txt"), STEP2_CHAIN)?;
    refs.insert(
        "C08-same-file-step2".into(),
        commit_ref(path, vcs, "C08-same-file-step2")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("tmp/transient.txt"), "temporary\n")?;
    refs.insert(
        "C09-add-then-delete-a".into(),
        commit_ref(path, vcs, "C09-add-then-delete-a")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    remove_tracked_file(path, vcs, "tmp/transient.txt")?;
    refs.insert(
        "C10-add-then-delete-b".into(),
        commit_ref(path, vcs, "C10-add-then-delete-b")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/empty.txt"), EMPTY_FILE)?;
    refs.insert(
        "C11-empty-add".into(),
        commit_ref(path, vcs, "C11-empty-add")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(
        path,
        vcs.source_rel("src/no_final_newline.txt"),
        NO_FINAL_NEWLINE,
    )?;
    refs.insert(
        "C12-no-final-newline".into(),
        commit_ref(path, vcs, "C12-no-final-newline")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    if vcs == VcsKind::Git {
        write_text(path, vcs.source_rel("src/中文路径.txt"), CHINESE_FILE)?;
    } else {
        svn(
            path,
            &[
                "propset",
                "relay-test",
                "yes",
                &vcs.source_rel("src/existing.txt"),
            ],
        )?;
    }
    refs.insert(
        "C13-chinese-add".into(),
        commit_ref(path, vcs, "C13-chinese-add")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("module-a/src/a.txt"), MODULE_FILE)?;
    refs.insert(
        "C14-longest-prefix".into(),
        commit_ref(path, vcs, "C14-longest-prefix")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    write_text(path, vcs.source_rel("src/mapped.txt"), "mapped\n")?;
    write_text(path, vcs.source_rel("docs/unmapped.txt"), "unmapped\n")?;
    refs.insert(
        "C15-partial-unmapped".into(),
        commit_ref(path, vcs, "C15-partial-unmapped")?,
    );
    if has_needed_refs(&refs, needed_refs) {
        return Ok(refs);
    }

    if vcs == VcsKind::Git {
        git(path, &["mv", "-f", "src/case.txt", "src/CASE.txt"])?;
        refs.insert(
            "C16-case-only-rename".into(),
            commit_ref(path, vcs, "C16-case-only-rename")?,
        );
    }

    Ok(refs)
}

fn has_needed_refs(refs: &BTreeMap<String, String>, needed_refs: &HashSet<&'static str>) -> bool {
    needed_refs.iter().all(|name| refs.contains_key(*name))
}

fn init_git(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)?;
    if command_status(None, "git", &["init", "-b", "main", path_str(path)]).is_err() {
        command(None, "git", &["init", path_str(path)])?;
        git(path, &["checkout", "-B", "main"])?;
    }
    git(
        path,
        &["config", "user.email", "relay-regression@example.invalid"],
    )?;
    git(path, &["config", "user.name", "Relay Regression"])?;
    git(path, &["config", "core.autocrlf", "false"])?;
    Ok(())
}

fn write_git_baseline(path: &Path) -> AppResult<()> {
    write_baseline(path, "")?;
    Ok(())
}

fn write_svn_baseline(path: &Path) -> AppResult<()> {
    write_baseline(path, "trunk/")?;
    Ok(())
}

fn write_baseline(path: &Path, prefix: &str) -> AppResult<()> {
    write_text(path, format!("{prefix}src/existing.txt"), BASE_EXISTING)?;
    write_text(path, format!("{prefix}src/delete_me.txt"), DELETE_ME)?;
    write_text(path, format!("{prefix}src/chain.txt"), BASE_CHAIN)?;
    write_text(path, format!("{prefix}src/target_only.txt"), TARGET_ONLY)?;
    write_text(path, format!("{prefix}src/move_me.txt"), MOVE_ME)?;
    write_text(path, format!("{prefix}src/case.txt"), CASE_FILE)?;
    write_text(path, format!("{prefix}README.md"), README)?;
    Ok(())
}

fn apply_target_setup(target: &Path, vcs: VcsKind, setup: TargetSetup) -> AppResult<()> {
    match setup {
        TargetSetup::Clean => {}
        TargetSetup::Dirty => {
            write_text(
                target,
                vcs.target_rel("src/target_only.txt"),
                "target\nonly\ndirty\n",
            )?;
        }
        TargetSetup::OccupiedNewFile => {
            write_text(target, vcs.target_rel("src/new_file.txt"), "already here\n")?;
            commit_target(target, vcs, "target has occupied path")?;
        }
        TargetSetup::ExistingDirectoryAtNewFile => {
            fs::create_dir_all(target.join(vcs.target_rel("src/new_file.txt")))?;
            write_text(
                target,
                vcs.target_rel("src/new_file.txt/nested.txt"),
                "nested\n",
            )?;
            commit_target(target, vcs, "target has directory at file path")?;
        }
        TargetSetup::AlreadyHasModify => {
            write_text(target, vcs.target_rel("src/existing.txt"), MOD_EXISTING)?;
            commit_target(target, vcs, "target already has expected modify")?;
        }
        TargetSetup::DriftedModify => {
            write_text(target, vcs.target_rel("src/existing.txt"), DRIFT_EXISTING)?;
            commit_target(target, vcs, "target has drifted modify context")?;
        }
        TargetSetup::SameRegionModify => {
            write_text(
                target,
                vcs.target_rel("src/existing.txt"),
                SAME_REGION_TARGET,
            )?;
            commit_target(target, vcs, "target has same region edit")?;
        }
        TargetSetup::MultiCandidateModify => {
            write_text(
                target,
                vcs.target_rel("src/existing.txt"),
                MULTI_CANDIDATE_EXISTING,
            )?;
            commit_target(target, vcs, "target has multiple candidate blocks")?;
        }
    }
    Ok(())
}

fn commit_ref(path: &Path, vcs: VcsKind, message: &str) -> AppResult<String> {
    match vcs {
        VcsKind::Git => {
            commit(path, message)?;
            Ok(format!("git:{}", git(path, &["rev-parse", "HEAD"])?.trim()))
        }
        VcsKind::Svn => {
            svn_add_new(path)?;
            let output = svn(path, &["commit", "-m", message])?;
            svn(path, &["update"])?;
            Ok(format!("svn:{}", parse_svn_commit_revision(&output)?))
        }
    }
}

fn commit_target(path: &Path, vcs: VcsKind, message: &str) -> AppResult<()> {
    match vcs {
        VcsKind::Git => commit(path, message),
        VcsKind::Svn => {
            svn_add_new(path)?;
            svn(path, &["commit", "-m", message])?;
            svn(path, &["update"])?;
            Ok(())
        }
    }
}

fn commit(path: &Path, message: &str) -> AppResult<()> {
    git(path, &["add", "-A"])?;
    git(path, &["commit", "-m", message])?;
    Ok(())
}

fn move_tracked_file(path: &Path, vcs: VcsKind, from: &str, to: &str) -> AppResult<()> {
    match vcs {
        VcsKind::Git => {
            let dst = path.join(to);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            git(path, &["mv", from, to])?;
        }
        VcsKind::Svn => {
            let from = vcs.source_rel(from);
            let to = vcs.source_rel(to);
            if let Some(parent) = Path::new(&to).parent() {
                let parent = parent.to_string_lossy().replace('\\', "/");
                fs::create_dir_all(path.join(&parent))?;
                svn_add_new(path)?;
            }
            svn(path, &["move", &from, &to])?;
        }
    }
    Ok(())
}

fn remove_tracked_file(path: &Path, vcs: VcsKind, rel: &str) -> AppResult<()> {
    match vcs {
        VcsKind::Git => remove_file(path, rel),
        VcsKind::Svn => {
            let rel = vcs.source_rel(rel);
            svn(path, &["delete", &rel])?;
            Ok(())
        }
    }
}

fn git(path: &Path, args: &[&str]) -> AppResult<String> {
    command(Some(path), "git", args)
}

fn svn(path: &Path, args: &[&str]) -> AppResult<String> {
    command(Some(path), "svn", args)
}

fn command(cwd: Option<&Path>, program: &str, args: &[&str]) -> AppResult<String> {
    if env::var("COPY_DIFF_REGRESSION_TRACE").is_ok() {
        let cwd = cwd
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".into());
        eprintln!("regression command: cwd={cwd} {program} {}", args.join(" "));
    }
    let mut cmd = copy_diff_lib::process::command(program);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.args(args);
    let output = copy_diff_lib::process::output(&mut cmd)
        .map_err(|e| AppError::Other(anyhow::anyhow!("{program}: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(AppError::Other(anyhow::anyhow!(
            "{program} {} failed: {stderr}{stdout}",
            args.join(" ")
        )))
    }
}

fn command_status(cwd: Option<&Path>, program: &str, args: &[&str]) -> AppResult<()> {
    command(cwd, program, args).map(|_| ())
}

fn command_available(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}

fn write_text(root: &Path, rel: impl AsRef<str>, content: &str) -> AppResult<()> {
    write_bytes(root, rel, content.as_bytes())
}

fn write_bytes(root: &Path, rel: impl AsRef<str>, content: &[u8]) -> AppResult<()> {
    let path = root.join(rel.as_ref().replace('/', std::path::MAIN_SEPARATOR_STR));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn remove_file(root: &Path, rel: &str) -> AppResult<()> {
    let path = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn svn_add(root: &Path, rel: &str) -> AppResult<()> {
    svn(root, &["add", rel])?;
    Ok(())
}

fn svn_add_new(root: &Path) -> AppResult<()> {
    let status = svn(root, &["status"])?;
    for line in status.lines() {
        let trimmed = line.trim();
        if let Some(path) = trimmed.strip_prefix('?').map(str::trim) {
            svn(root, &["add", "--parents", path])?;
        }
    }
    Ok(())
}

fn write_manifest(root: &Path, out_dir: &Path) -> AppResult<Manifest> {
    fs::create_dir_all(out_dir)?;
    let manifest = manifest(root)?;
    write_json(&out_dir.join("manifest.json"), &manifest)?;
    Ok(manifest)
}

fn manifest(root: &Path) -> AppResult<Manifest> {
    let mut files = BTreeMap::new();
    collect_manifest(root, root, &mut files)?;
    Ok(Manifest { files })
}

fn collect_manifest(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str());
        if name == Some(".git") || name == Some(".svn") || name == Some("repo") {
            continue;
        }
        if path.is_dir() {
            collect_manifest(root, &path, files)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path)?;
            files.insert(rel, sha256_hex(&bytes));
        }
    }
    Ok(())
}

fn write_target_diff(target: &Path, vcs: VcsKind, baseline: &str, out: &Path) {
    let diff = match vcs {
        VcsKind::Git => git(target, &["diff", baseline, "HEAD"]),
        VcsKind::Svn => svn(target, &["diff", "-r", &format!("{baseline}:HEAD")]),
    };
    if let Ok(diff) = diff {
        let _ = fs::write(out, diff);
    }
}

fn target_head(target: &Path, vcs: VcsKind) -> AppResult<String> {
    match vcs {
        VcsKind::Git => git(target, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string()),
        VcsKind::Svn => {
            svn(target, &["info", "--show-item", "revision"]).map(|s| s.trim().to_string())
        }
    }
}

fn target_status(target: &Path, vcs: VcsKind) -> AppResult<String> {
    match vcs {
        VcsKind::Git => git(target, &["status", "--porcelain"]),
        VcsKind::Svn => svn(target, &["status", "--ignore-externals"]),
    }
}

fn target_commit_count(target: &Path, vcs: VcsKind, baseline: &str) -> AppResult<usize> {
    match vcs {
        VcsKind::Git => git(
            target,
            &["rev-list", "--count", &format!("{baseline}..HEAD")],
        )
        .map(|s| s.trim().parse::<usize>().unwrap_or(usize::MAX)),
        VcsKind::Svn => {
            let head = target_head(target, vcs)?;
            let base = baseline.parse::<usize>().unwrap_or(usize::MAX);
            let head = head.parse::<usize>().unwrap_or(usize::MAX);
            Ok(head.saturating_sub(base))
        }
    }
}

fn target_commit_messages(target: &Path, vcs: VcsKind, baseline: &str) -> AppResult<Vec<String>> {
    match vcs {
        VcsKind::Git => {
            let range = format!("{baseline}..HEAD");
            let output = git(target, &["log", "--reverse", "--format=%B%x1e", &range])?;
            Ok(output
                .split('\x1e')
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .map(ToOwned::to_owned)
                .collect())
        }
        VcsKind::Svn => {
            let head = target_head(target, vcs)?;
            let base = baseline.parse::<usize>().unwrap_or(usize::MAX);
            let head = head.parse::<usize>().unwrap_or(usize::MAX);
            if head <= base {
                return Ok(Vec::new());
            }
            let range = format!("{}:HEAD", base + 1);
            let output = svn(target, &["log", "--xml", "-r", &range])?;
            let mut entries = parse_log_xml(&output)?;
            entries.reverse();
            Ok(entries.into_iter().map(|entry| entry.message).collect())
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn manifest_delta(before: &Manifest, after: &Manifest) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (path, hash) in &after.files {
        if before.files.get(path) != Some(hash) {
            out.insert(path.clone(), "changed".into());
        }
    }
    for path in before.files.keys() {
        if !after.files.contains_key(path) {
            out.insert(path.clone(), "deleted".into());
        }
    }
    out
}

fn expected_delta(
    changed: &BTreeMap<&'static str, ExpectedContent>,
    deleted: &[&'static str],
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for path in changed.keys() {
        out.insert((*path).into(), "changed".into());
    }
    for path in deleted {
        out.insert((*path).into(), "deleted".into());
    }
    out
}

fn repo_record(id: &str, path: &Path, vcs: VcsKind) -> RepoRecord {
    RepoRecord {
        id: id.into(),
        name: id.into(),
        path: path.to_string_lossy().into_owned(),
        repo_type: vcs.repo_type(),
        branch: vcs.branch().into(),
        last_used: None,
        svn_user: None,
        svn_pass_encrypted: None,
        path_mappings: vec![],
    }
}

fn migration_record(
    source: &RepoRecord,
    target: &RepoRecord,
    target_branch: &str,
    units: &[copy_diff_lib::model::PreviewUnit],
    aggregated: &[FileChange],
) -> MigrationRecord {
    let commits = units
        .iter()
        .map(|unit| {
            let rev = unit
                .meta
                .source_ref
                .strip_prefix("svn:")
                .or_else(|| unit.meta.source_ref.strip_prefix("git:"))
                .unwrap_or(&unit.meta.source_ref);
            CommitSnapshot {
                id: unit.meta.source_ref.clone(),
                hash: rev.chars().take(7).collect(),
                msg: unit.meta.message.clone(),
                author: unit.meta.author.clone(),
                date: unit.meta.date.clone(),
                files: unit.files.len(),
            }
        })
        .collect::<Vec<_>>();
    let files = aggregated
        .iter()
        .map(preview_file_with_diff)
        .collect::<Vec<FileChangeView>>();
    let comparison_files = migration_file_pairs(aggregated);
    MigrationRecord {
        id: format!("m{}", chrono::Utc::now().timestamp_millis()),
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
            branch: target_branch.to_string(),
        },
        commits,
        files,
        comparison_files,
        conflicts_resolved: 0,
        status: "success".into(),
    }
}

fn content_equal(a: Option<&str>, b: Option<&str>) -> bool {
    let norm = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
    norm(a.unwrap_or("")) == norm(b.unwrap_or(""))
}

fn cases_for_suite(suite: Suite) -> Vec<CaseSpec> {
    let mut cases = git_core_cases();
    if suite == Suite::Smoke {
        cases.retain(|c| matches!(c.id, "I01-GG" | "I03-GG" | "I04-GG"));
        return cases;
    }

    cases.extend(git_safety_cases());
    if suite == Suite::Release {
        cases.extend(vcs_matrix_cases());
    }
    cases
}

fn base_case(
    id: &'static str,
    source: VcsKind,
    target: VcsKind,
    commits: Vec<&'static str>,
) -> CaseSpec {
    CaseSpec {
        id,
        source,
        target,
        commits,
        mappings: default_mapping(source, target),
        target_setup: TargetSetup::Clean,
        migration_mode: MigrationMode::IncrementalFirst,
        squash_commits: false,
        squash_message: None,
        expect: Expectation::Success {
            changed: BTreeMap::new(),
            deleted: vec![],
            absent: vec![],
            unchanged: vec![],
            commit_count: 0,
        },
        accept_review: false,
        resolved_blocked: vec![],
        checks_history: false,
    }
}

fn git_core_cases() -> Vec<CaseSpec> {
    vec![
        success_case(
            "I01-GG",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C02-modify-text"],
            changed(&[("src/existing.txt", ExpectedContent::Text(MOD_EXISTING))]),
            vec![],
            1,
        ),
        failure_case(
            "I02-GG",
            vec!["C02-modify-text"],
            TargetSetup::Dirty,
            "execute",
            "not clean",
        ),
        {
            let mut c = failure_case(
                "I03-GG",
                vec!["C01-add-text"],
                TargetSetup::Clean,
                "preview",
                "mapping",
            );
            c.mappings = vec![MappingInput {
                from: "unmatched".into(),
                to: ".".into(),
            }];
            c
        },
        success_case(
            "I04-GG",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C07-same-file-step1", "C08-same-file-step2"],
            changed(&[("src/chain.txt", ExpectedContent::Text(STEP2_CHAIN))]),
            vec![],
            2,
        ),
        {
            let mut c = success_case(
                "I05-GG",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C09-add-then-delete-a", "C10-add-then-delete-b"],
                BTreeMap::new(),
                vec![],
                2,
            );
            set_absent(&mut c, vec!["tmp/transient.txt"]);
            c
        },
        {
            let mut c = success_case(
                "I06-GG",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C03-delete-text"],
                BTreeMap::new(),
                vec!["src/delete_me.txt"],
                1,
            );
            c.accept_review = true;
            c
        },
        failure_case(
            "I07-GG",
            vec!["C01-add-text"],
            TargetSetup::OccupiedNewFile,
            "execute",
            "blocked",
        ),
        failure_case(
            "I08-GG",
            vec!["C01-add-text"],
            TargetSetup::ExistingDirectoryAtNewFile,
            "execute",
            "apply failed",
        ),
    ]
}

fn git_safety_cases() -> Vec<CaseSpec> {
    let mut out = vec![
        success_case(
            "D01-GG",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C01-add-text"],
            changed(&[("src/new_file.txt", ExpectedContent::Text(NEW_FILE))]),
            vec![],
            1,
        ),
        {
            let mut c = success_case(
                "D03-GG",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C03-delete-text"],
                BTreeMap::new(),
                vec!["src/delete_me.txt"],
                1,
            );
            c.accept_review = true;
            c
        },
        success_case(
            "D04-GG-rename",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C04-rename-text"],
            changed(&[("moved/move_me.txt", ExpectedContent::Text(MOVE_ME))]),
            vec!["src/move_me.txt"],
            1,
        ),
        success_case(
            "D05-GG-binary",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C05-binary-add"],
            changed(&[("assets/blob.bin", ExpectedContent::Bytes(BINARY_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "D06-GG-crlf",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C06-crlf-add"],
            changed(&[("src/crlf.txt", ExpectedContent::Text(CRLF_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "D07-GG-empty",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C11-empty-add"],
            changed(&[("src/empty.txt", ExpectedContent::Text(EMPTY_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "D08-GG-no-final-newline",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C12-no-final-newline"],
            changed(&[(
                "src/no_final_newline.txt",
                ExpectedContent::Text(NO_FINAL_NEWLINE),
            )]),
            vec![],
            1,
        ),
        success_case(
            "D09-GG-chinese",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C13-chinese-add"],
            changed(&[("src/中文路径.txt", ExpectedContent::Text(CHINESE_FILE))]),
            vec![],
            1,
        ),
        {
            let mut c = success_case(
                "D10-GG-longest-prefix",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C14-longest-prefix"],
                changed(&[(
                    "packages/module-a/src/a.txt",
                    ExpectedContent::Text(MODULE_FILE),
                )]),
                vec![],
                1,
            );
            c.mappings = vec![
                MappingInput {
                    from: "module-a".into(),
                    to: "packages/module-a".into(),
                },
                MappingInput {
                    from: ".".into(),
                    to: ".".into(),
                },
            ];
            c
        },
        {
            let mut c = failure_case(
                "E01-GG-partial-unmapped",
                vec!["C15-partial-unmapped"],
                TargetSetup::Clean,
                "preview",
                "mapping",
            );
            c.mappings = vec![MappingInput {
                from: "src".into(),
                to: "src".into(),
            }];
            c
        },
        failure_case(
            "E02-GG-directory-conflict",
            vec!["C01-add-text"],
            TargetSetup::ExistingDirectoryAtNewFile,
            "execute",
            "apply failed",
        ),
        {
            let mut c = success_case(
                "M01-GG-commit-result",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C07-same-file-step1", "C08-same-file-step2"],
                changed(&[("src/chain.txt", ExpectedContent::Text(STEP2_CHAIN))]),
                vec![],
                3,
            );
            c.migration_mode = MigrationMode::CommitResult;
            c.accept_review = true;
            c
        },
        {
            let mut c = success_case(
                "M02-GG-strict-replay",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C01-add-text"],
                changed(&[("src/new_file.txt", ExpectedContent::Text(NEW_FILE))]),
                vec![],
                1,
            );
            c.migration_mode = MigrationMode::StrictReplay;
            c
        },
        {
            let mut c = success_case(
                "M03-GG-squash",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C01-add-text", "C02-modify-text", "C03-delete-text"],
                changed(&[
                    ("src/new_file.txt", ExpectedContent::Text(NEW_FILE)),
                    ("src/existing.txt", ExpectedContent::Text(MOD_EXISTING)),
                ]),
                vec!["src/delete_me.txt"],
                1,
            );
            c.accept_review = true;
            c.squash_commits = true;
            c.squash_message = Some("relay regression squash");
            c
        },
        {
            let mut c = failure_case(
                "M04-GG-squash-empty-message",
                vec!["C01-add-text"],
                TargetSetup::Clean,
                "execute",
                "message",
            );
            c.squash_commits = true;
            c.squash_message = Some(" ");
            c
        },
        {
            let mut c = success_case(
                "F10-GG-already-expected",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C02-modify-text"],
                BTreeMap::new(),
                vec![],
                0,
            );
            c.target_setup = TargetSetup::AlreadyHasModify;
            set_unchanged(&mut c, vec!["src/existing.txt"]);
            c.checks_history = true;
            c
        },
        {
            let mut c = success_case(
                "F11-GG-context-drift-auto-merge",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C02-modify-text"],
                changed(&[(
                    "src/existing.txt",
                    ExpectedContent::Text(DRIFT_MOD_EXISTING),
                )]),
                vec![],
                1,
            );
            c.target_setup = TargetSetup::DriftedModify;
            c.accept_review = true;
            c
        },
        {
            let mut c = success_case(
                "F12-GG-already-contained-skip",
                VcsKind::Git,
                VcsKind::Git,
                vec!["C02-modify-text"],
                BTreeMap::new(),
                vec![],
                0,
            );
            c.target_setup = TargetSetup::AlreadyHasModify;
            set_unchanged(&mut c, vec!["src/existing.txt"]);
            c
        },
        {
            failure_case(
                "F13-GG-same-region-conflict",
                vec!["C02-modify-text"],
                TargetSetup::SameRegionModify,
                "execute",
                "blocked",
            )
        },
        {
            failure_case(
                "F14-GG-multiple-candidates-blocked",
                vec!["C02-modify-text"],
                TargetSetup::MultiCandidateModify,
                "execute",
                "blocked",
            )
        },
    ];
    if cfg!(windows) {
        out.push(success_case(
            "D11-GG-case-only-rename",
            VcsKind::Git,
            VcsKind::Git,
            vec!["C16-case-only-rename"],
            changed(&[("src/CASE.txt", ExpectedContent::Text(CASE_FILE))]),
            vec!["src/case.txt"],
            1,
        ));
    }
    out
}

fn vcs_matrix_cases() -> Vec<CaseSpec> {
    vec![
        success_case(
            "SG01-SVN-Git-add",
            VcsKind::Svn,
            VcsKind::Git,
            vec!["C01-add-text"],
            changed(&[("src/new_file.txt", ExpectedContent::Text(NEW_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "SG02-SVN-Git-modify",
            VcsKind::Svn,
            VcsKind::Git,
            vec!["C02-modify-text"],
            changed(&[("src/existing.txt", ExpectedContent::Text(MOD_EXISTING))]),
            vec![],
            1,
        ),
        success_case(
            "SG03-SVN-Git-move",
            VcsKind::Svn,
            VcsKind::Git,
            vec!["C04-rename-text"],
            changed(&[("moved/move_me.txt", ExpectedContent::Text(MOVE_ME))]),
            vec!["src/move_me.txt"],
            1,
        )
        .with_accept_review(),
        {
            let mut c = success_case(
                "SG04-SVN-Git-multi",
                VcsKind::Svn,
                VcsKind::Git,
                vec!["C01-add-text", "C02-modify-text", "C03-delete-text"],
                changed(&[
                    ("src/new_file.txt", ExpectedContent::Text(NEW_FILE)),
                    ("src/existing.txt", ExpectedContent::Text(MOD_EXISTING)),
                ]),
                vec!["src/delete_me.txt"],
                3,
            );
            c.accept_review = true;
            c.checks_history = true;
            c
        },
        success_case(
            "GS01-Git-SVN-add",
            VcsKind::Git,
            VcsKind::Svn,
            vec!["C01-add-text"],
            changed(&[("trunk/src/new_file.txt", ExpectedContent::Text(NEW_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "GS02-Git-SVN-delete",
            VcsKind::Git,
            VcsKind::Svn,
            vec!["C03-delete-text"],
            BTreeMap::new(),
            vec!["trunk/src/delete_me.txt"],
            1,
        )
        .with_accept_review(),
        {
            let mut c = base_case(
                "GS03-Git-SVN-dirty",
                VcsKind::Git,
                VcsKind::Svn,
                vec!["C01-add-text"],
            );
            c.target_setup = TargetSetup::Dirty;
            c.expect = Expectation::Failure {
                phase: "execute",
                contains: "not clean",
                target_unchanged: true,
            };
            c
        },
        success_case(
            "SS01-SVN-SVN-add",
            VcsKind::Svn,
            VcsKind::Svn,
            vec!["C01-add-text"],
            changed(&[("trunk/src/new_file.txt", ExpectedContent::Text(NEW_FILE))]),
            vec![],
            1,
        ),
        success_case(
            "SS02-SVN-SVN-delete",
            VcsKind::Svn,
            VcsKind::Svn,
            vec!["C03-delete-text"],
            BTreeMap::new(),
            vec!["trunk/src/delete_me.txt"],
            1,
        )
        .with_accept_review(),
    ]
}

fn success_case(
    id: &'static str,
    source: VcsKind,
    target: VcsKind,
    commits: Vec<&'static str>,
    changed: BTreeMap<&'static str, ExpectedContent>,
    deleted: Vec<&'static str>,
    commit_count: usize,
) -> CaseSpec {
    let mut c = base_case(id, source, target, commits);
    c.expect = Expectation::Success {
        changed,
        deleted,
        absent: vec![],
        unchanged: default_unchanged(target),
        commit_count,
    };
    c
}

fn failure_case(
    id: &'static str,
    commits: Vec<&'static str>,
    target_setup: TargetSetup,
    phase: &'static str,
    contains: &'static str,
) -> CaseSpec {
    let mut c = base_case(id, VcsKind::Git, VcsKind::Git, commits);
    c.target_setup = target_setup;
    c.expect = Expectation::Failure {
        phase,
        contains,
        target_unchanged: true,
    };
    c
}

fn default_mapping(source: VcsKind, target: VcsKind) -> Vec<MappingInput> {
    vec![MappingInput {
        from: match source {
            VcsKind::Git => ".".into(),
            VcsKind::Svn => "/trunk".into(),
        },
        to: match target {
            VcsKind::Git => ".".into(),
            VcsKind::Svn => "trunk".into(),
        },
    }]
}

fn default_unchanged(target: VcsKind) -> Vec<&'static str> {
    match target {
        VcsKind::Git => vec!["README.md", "src/target_only.txt"],
        VcsKind::Svn => vec!["trunk/README.md", "trunk/src/target_only.txt"],
    }
}

fn changed(entries: &[(&'static str, ExpectedContent)]) -> BTreeMap<&'static str, ExpectedContent> {
    entries.iter().cloned().collect()
}

fn set_absent(case: &mut CaseSpec, paths: Vec<&'static str>) {
    if let Expectation::Success { absent, .. } = &mut case.expect {
        *absent = paths;
    }
}

fn set_unchanged(case: &mut CaseSpec, paths: Vec<&'static str>) {
    if let Expectation::Success { unchanged, .. } = &mut case.expect {
        unchanged.extend(paths);
    }
}

fn case_to_json(case: &CaseSpec) -> serde_json::Value {
    serde_json::json!({
        "id": case.id,
        "sourceType": case.source.name(),
        "targetType": case.target.name(),
        "sourceCommits": case.commits,
        "pathMappings": case.mappings.iter().map(|m| {
            serde_json::json!({"from": m.from, "to": m.to})
        }).collect::<Vec<_>>(),
        "migrationMode": mode_name(case.migration_mode),
        "squashCommits": case.squash_commits,
        "targetSetup": format!("{:?}", case.target_setup),
    })
}

fn error_record(case_id: &str, err: &PhaseError, case_dir: &Path) -> ErrorRecord {
    let mut artifacts = BTreeMap::new();
    artifacts.insert("caseDir".into(), case_dir.to_string_lossy().into_owned());
    artifacts.insert("case".into(), "case.json".into());
    artifacts.insert("error".into(), "error.json".into());
    artifacts.insert("beforeManifest".into(), "before/manifest.json".into());
    artifacts.insert("afterManifest".into(), "after/manifest.json".into());
    artifacts.insert("targetDiff".into(), "after/target.diff".into());
    ErrorRecord {
        case_id: case_id.into(),
        phase: err.phase.into(),
        error_type: err.error_type.into(),
        message: err.message.clone(),
        expected: serde_json::json!({}),
        actual: serde_json::json!({}),
        artifacts,
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json =
        serde_json::to_string_pretty(value).map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    fs::write(path, json)?;
    Ok(())
}

fn write_summary_md(path: &Path, summary: &Summary) -> AppResult<()> {
    let mut out = format!(
        "# Regression {}\n\nSuite: `{}`\n\nPassed: {}\nFailed: {}\nSkipped: {}\n\n",
        summary.run_id, summary.suite, summary.passed, summary.failed, summary.skipped
    );
    out.push_str("| Case | Status | Message |\n|---|---|---|\n");
    for case in &summary.cases {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            case.case_id,
            case.status,
            case.message.replace('|', "\\|")
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn phase(phase: &'static str, error_type: &'static str, error: AppError) -> PhaseError {
    PhaseError {
        phase,
        error_type,
        message: error.to_string(),
    }
}

fn suite_name(suite: Suite) -> &'static str {
    match suite {
        Suite::Smoke => "smoke",
        Suite::ProductionSafety => "production-safety",
        Suite::Release => "release",
        Suite::UiSmoke => "ui-smoke",
    }
}

fn mode_name(mode: MigrationMode) -> &'static str {
    match mode {
        MigrationMode::IncrementalFirst => "incremental_first",
        MigrationMode::CommitResult => "commit_result",
        MigrationMode::StrictReplay => "strict_replay",
    }
}

fn new_run_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("run-{ts}")
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap_or(".")
}

fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn parse_svn_commit_revision(output: &str) -> AppResult<String> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Committed revision ") {
            return Ok(rest.trim_end_matches('.').to_string());
        }
    }
    Err(AppError::Vcs(format!(
        "could not parse svn commit revision from {output:?}"
    )))
}

impl VcsKind {
    fn name(self) -> &'static str {
        match self {
            VcsKind::Git => "git",
            VcsKind::Svn => "svn",
        }
    }

    fn repo_type(self) -> RepoType {
        match self {
            VcsKind::Git => RepoType::Git,
            VcsKind::Svn => RepoType::Svn,
        }
    }

    fn branch(self) -> &'static str {
        match self {
            VcsKind::Git => "main",
            VcsKind::Svn => "trunk",
        }
    }

    fn source_rel(self, rel: &str) -> String {
        match self {
            VcsKind::Git => rel.into(),
            VcsKind::Svn => format!("trunk/{rel}"),
        }
    }

    fn target_rel(self, rel: &str) -> String {
        match self {
            VcsKind::Git => rel.into(),
            VcsKind::Svn => format!("trunk/{rel}"),
        }
    }
}

impl ExpectedContent {
    fn sha256(&self) -> String {
        match self {
            ExpectedContent::Text(text) => sha256_hex(text.as_bytes()),
            ExpectedContent::Bytes(bytes) => sha256_hex(bytes),
        }
    }
}

trait RepoTypeName {
    fn name(&self) -> &'static str;
}

impl RepoTypeName for RepoType {
    fn name(&self) -> &'static str {
        match self {
            RepoType::Git => "git",
            RepoType::Svn => "svn",
        }
    }
}
