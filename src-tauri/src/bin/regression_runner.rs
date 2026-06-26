use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use copy_diff_lib::{
    error::{AppError, Result as AppResult},
    model::{ApplyStatus, ChangeSet},
    preview::{
        build_integration_plan, build_preview_plan_parallel, strategy_map, MappingInput,
        PreviewContext,
    },
    store::models::{IntegrationStatus, MigrationMode, RepoRecord, RepoType},
    vcs::MigrationWriter,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const BASE_EXISTING: &str = "alpha\nold\nomega\n";
const MOD_EXISTING: &str = "alpha\nnew\nomega\n";
const BASE_CHAIN: &str = "one\nbase\nthree\n";
const STEP1_CHAIN: &str = "one\nstep1\nthree\n";
const STEP2_CHAIN: &str = "one\nstep2\nthree\n";
const NEW_FILE: &str = "new\nfile\ncontent\n";
const DELETE_ME: &str = "delete\nme\n";
const TARGET_ONLY: &str = "target\nonly\n";
const README: &str = "relay regression target\n";

#[derive(Debug, Clone, Copy)]
enum Suite {
    Smoke,
    ProductionSafety,
    Release,
}

#[derive(Debug, Clone)]
struct CaseSpec {
    id: &'static str,
    commits: Vec<&'static str>,
    mappings: Vec<MappingInput>,
    target_setup: TargetSetup,
    expect: Expectation,
    accept_review: bool,
}

#[derive(Debug, Clone, Copy)]
enum TargetSetup {
    Clean,
    Dirty,
    OccupiedNewFile,
    FailingCommitHook,
    AlreadyHasModify,
}

#[derive(Debug, Clone)]
enum Expectation {
    Success {
        changed: BTreeMap<&'static str, &'static str>,
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

fn main() {
    let cfg = Config::from_args();
    let result = run_suite(&cfg);
    match result {
        Ok(summary) => {
            println!(
                "regression {}: {} passed, {} failed, {} skipped",
                summary.run_id, summary.passed, summary.failed, summary.skipped
            );
            if summary.failed > 0 {
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
}

impl Config {
    fn from_args() -> Self {
        let mut suite = Suite::ProductionSafety;
        let mut out_dir = PathBuf::from("../artifacts/regression");
        let mut keep_passed = false;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--suite" => {
                    let value = args.next().unwrap_or_else(|| "production-safety".into());
                    suite = match value.as_str() {
                        "smoke" => Suite::Smoke,
                        "production-safety" | "safety" => Suite::ProductionSafety,
                        "release" => Suite::Release,
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
                _ => {}
            }
        }
        Self {
            suite,
            out_dir,
            keep_passed,
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

    for case in cases_for_suite(cfg.suite) {
        let case_result = run_case(&run_dir, &case);
        match case_result {
            Ok(result) => {
                if result.status == "passed" {
                    summary.passed += 1;
                    if !cfg.keep_passed {
                        let _ = fs::remove_dir_all(&result.case_dir);
                    }
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

    write_json(&run_dir.join("summary.json"), &summary)?;
    write_summary_md(&run_dir.join("summary.md"), &summary)?;
    Ok(summary)
}

fn run_case(run_dir: &Path, case: &CaseSpec) -> AppResult<CaseResult> {
    let case_dir = run_dir.join("cases").join(case.id);
    fs::create_dir_all(&case_dir)?;
    write_json(&case_dir.join("case.json"), &case_to_json(case))?;

    let work = case_dir.join("work");
    let source = work.join("source-git");
    let target = work.join("target-git");
    fs::create_dir_all(&work)?;

    let refs = create_source_repo(&source)?;
    create_target_repo(&target)?;
    apply_target_setup(&target, case.target_setup)?;
    let baseline = git(&target, &["rev-parse", "HEAD"])?.trim().to_string();

    write_json(&case_dir.join("refs.json"), &refs)?;
    fs::write(case_dir.join("target_baseline_ref.txt"), &baseline)?;
    let before = write_manifest(&target, &case_dir.join("before"))?;

    let source_refs: Vec<String> = case
        .commits
        .iter()
        .map(|name| {
            refs.get(*name)
                .cloned()
                .ok_or_else(|| AppError::Other(anyhow::anyhow!("missing ref {name}")))
        })
        .collect::<AppResult<Vec<_>>>()?;

    let migration = relay_migrate(&source, &target, source_refs, case);
    let after = write_manifest(&target, &case_dir.join("after"))?;
    write_target_diff(
        &target,
        &baseline,
        &case_dir.join("after").join("target.diff"),
    );

    let result = match (&case.expect, migration) {
        (Expectation::Success { .. }, Ok(())) => {
            verify_success(case, &target, &baseline, &before, &after)
        }
        (Expectation::Success { .. }, Err(err)) => Err(err),
        (Expectation::Failure { phase, .. }, Ok(())) => Err(PhaseError {
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

fn relay_migrate(
    source_path: &Path,
    target_path: &Path,
    source_refs: Vec<String>,
    case: &CaseSpec,
) -> std::result::Result<(), PhaseError> {
    let source = repo_record("source", source_path);
    let target = repo_record("target", target_path);
    let ctx = PreviewContext::from_repos(&source, &target, None, case.mappings.clone())
        .map_err(|e| phase("preview", "unexpected_failure", e))?;
    let preview = build_preview_plan_parallel(&ctx, &source_refs)
        .map_err(|e| phase("preview", "unexpected_failure", e))?;
    let plan = build_integration_plan(
        &preview.aggregated,
        &target.path,
        ctx.target_kind,
        MigrationMode::IncrementalFirst,
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
    validate_plan(&plan, &accepted_review, &[])
        .map_err(|e| phase("execute", "unexpected_failure", e))?;

    let writer = MigrationWriter::from_repo(&target, None)
        .map_err(|e| phase("execute", "unexpected_failure", e))?;
    let checkpoint = writer
        .prepare(&target.branch)
        .map_err(|e| phase("execute", "unexpected_failure", e))?;

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
                    unit_apply_strategy(strategy, status, fc.patch.is_some()),
                ))
            })
            .collect::<HashMap<_, _>>();
        let apply = writer
            .apply_changeset_with_strategies(
                &ChangeSet {
                    meta: unit.meta.clone(),
                    files: unit.files.clone(),
                },
                &unit_strategies,
            )
            .map_err(|e| phase("execute", "unexpected_failure", e))?;
        if apply.status != ApplyStatus::Ok {
            let _ = writer.rollback(&checkpoint);
            return Err(PhaseError {
                phase: "execute",
                error_type: "unexpected_failure",
                message: format!("apply failed: {:?}", apply.failed_paths),
            });
        }
        if let Err(err) = writer.commit_allow_empty(&unit.meta, "relay: {message}") {
            let _ = writer.rollback(&checkpoint);
            return Err(phase("execute", "unexpected_failure", err));
        }
    }
    Ok(())
}

fn unit_apply_strategy(
    strategy: copy_diff_lib::store::models::IntegrationStrategy,
    status: IntegrationStatus,
    has_patch: bool,
) -> copy_diff_lib::store::models::IntegrationStrategy {
    if status == IntegrationStatus::AutoOk
        && strategy == copy_diff_lib::store::models::IntegrationStrategy::WriteAfter
        && has_patch
    {
        copy_diff_lib::store::models::IntegrationStrategy::ApplyPatch
    } else {
        strategy
    }
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
        let expected_hash = sha256_hex(expected_content.as_bytes());
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

    let status = git(target, &["status", "--porcelain"])
        .map_err(|e| phase("verify", "dirty_worktree", e))?;
    if !status.trim().is_empty() {
        return Err(PhaseError {
            phase: "verify",
            error_type: "dirty_worktree",
            message: status,
        });
    }

    let count = git(
        target,
        &["rev-list", "--count", &format!("{baseline}..HEAD")],
    )
    .map_err(|e| phase("verify", "commit_count_mismatch", e))?
    .trim()
    .parse::<usize>()
    .unwrap_or(usize::MAX);
    if count != *commit_count {
        return Err(PhaseError {
            phase: "verify",
            error_type: "commit_count_mismatch",
            message: format!("expected {commit_count} commits, got {count}"),
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

fn create_source_repo(path: &Path) -> AppResult<BTreeMap<String, String>> {
    init_git(path)?;
    write_baseline(path)?;
    commit(path, "baseline")?;
    let mut refs = BTreeMap::new();

    write_file(path, "src/new_file.txt", NEW_FILE)?;
    refs.insert("C01-add-text".into(), commit_ref(path, "C01-add-text")?);

    write_file(path, "src/existing.txt", MOD_EXISTING)?;
    refs.insert(
        "C02-modify-text".into(),
        commit_ref(path, "C02-modify-text")?,
    );

    remove_file(path, "src/delete_me.txt")?;
    refs.insert(
        "C03-delete-text".into(),
        commit_ref(path, "C03-delete-text")?,
    );

    write_file(path, "src/chain.txt", STEP1_CHAIN)?;
    refs.insert(
        "C07-same-file-step1".into(),
        commit_ref(path, "C07-same-file-step1")?,
    );

    write_file(path, "src/chain.txt", STEP2_CHAIN)?;
    refs.insert(
        "C08-same-file-step2".into(),
        commit_ref(path, "C08-same-file-step2")?,
    );

    write_file(path, "tmp/transient.txt", "temporary\n")?;
    refs.insert(
        "C09-add-then-delete-a".into(),
        commit_ref(path, "C09-add-then-delete-a")?,
    );

    remove_file(path, "tmp/transient.txt")?;
    refs.insert(
        "C10-add-then-delete-b".into(),
        commit_ref(path, "C10-add-then-delete-b")?,
    );

    Ok(refs)
}

fn create_target_repo(path: &Path) -> AppResult<String> {
    init_git(path)?;
    write_baseline(path)?;
    commit(path, "baseline")?;
    Ok(git(path, &["rev-parse", "HEAD"])?.trim().to_string())
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

fn write_baseline(path: &Path) -> AppResult<()> {
    write_file(path, "src/existing.txt", BASE_EXISTING)?;
    write_file(path, "src/delete_me.txt", DELETE_ME)?;
    write_file(path, "src/chain.txt", BASE_CHAIN)?;
    write_file(path, "src/target_only.txt", TARGET_ONLY)?;
    write_file(path, "README.md", README)?;
    Ok(())
}

fn apply_target_setup(target: &Path, setup: TargetSetup) -> AppResult<()> {
    match setup {
        TargetSetup::Clean => {}
        TargetSetup::Dirty => {
            write_file(target, "src/target_only.txt", "target\nonly\ndirty\n")?;
        }
        TargetSetup::OccupiedNewFile => {
            write_file(target, "src/new_file.txt", "already here\n")?;
            commit(target, "target has occupied path")?;
        }
        TargetSetup::FailingCommitHook => {
            let hook = target.join(".git").join("hooks").join("pre-commit");
            fs::write(hook, "#!/bin/sh\nexit 1\n")?;
        }
        TargetSetup::AlreadyHasModify => {
            write_file(target, "src/existing.txt", MOD_EXISTING)?;
            commit(target, "target already has expected modify")?;
        }
    }
    Ok(())
}

fn commit_ref(path: &Path, message: &str) -> AppResult<String> {
    commit(path, message)?;
    Ok(format!("git:{}", git(path, &["rev-parse", "HEAD"])?.trim()))
}

fn commit(path: &Path, message: &str) -> AppResult<()> {
    git(path, &["add", "-A"])?;
    git(path, &["commit", "-m", message])?;
    Ok(())
}

fn git(path: &Path, args: &[&str]) -> AppResult<String> {
    command(Some(path), "git", args)
}

fn command(cwd: Option<&Path>, program: &str, args: &[&str]) -> AppResult<String> {
    let mut cmd = Command::new(program);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd
        .args(args)
        .output()
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

fn write_file(root: &Path, rel: &str, content: &str) -> AppResult<()> {
    let path = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
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
        if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
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

fn write_target_diff(target: &Path, baseline: &str, out: &Path) {
    if let Ok(diff) = git(target, &["diff", baseline, "HEAD"]) {
        let _ = fs::write(out, diff);
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
    changed: &BTreeMap<&'static str, &'static str>,
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

fn repo_record(id: &str, path: &Path) -> RepoRecord {
    RepoRecord {
        id: id.into(),
        name: id.into(),
        path: path.to_string_lossy().into_owned(),
        repo_type: RepoType::Git,
        branch: "main".into(),
        last_used: None,
        svn_user: None,
        svn_pass_encrypted: None,
        path_mappings: vec![],
    }
}

fn cases_for_suite(suite: Suite) -> Vec<CaseSpec> {
    let mut cases = vec![
        case_i01(),
        case_i02(),
        case_i03(),
        case_i04(),
        case_i05(),
        case_i06(),
        case_i07(),
        case_i08(),
    ];
    if matches!(suite, Suite::Smoke) {
        cases.retain(|c| matches!(c.id, "I01-GG" | "I03-GG" | "I04-GG"));
    } else if matches!(suite, Suite::Release) {
        cases.extend([
            case_d01(),
            case_d03(),
            case_f05(),
            case_e07(),
            case_f10(),
        ]);
    }
    cases
}

fn default_mapping() -> Vec<MappingInput> {
    vec![MappingInput {
        from: ".".into(),
        to: ".".into(),
    }]
}

fn bad_mapping() -> Vec<MappingInput> {
    vec![MappingInput {
        from: "unmatched".into(),
        to: ".".into(),
    }]
}

fn changed(entries: &[(&'static str, &'static str)]) -> BTreeMap<&'static str, &'static str> {
    entries.iter().copied().collect()
}

fn case_i01() -> CaseSpec {
    CaseSpec {
        id: "I01-GG",
        commits: vec!["C02-modify-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: changed(&[("src/existing.txt", MOD_EXISTING)]),
            deleted: vec![],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 1,
        },
        accept_review: false,
    }
}

fn case_i02() -> CaseSpec {
    CaseSpec {
        id: "I02-GG",
        commits: vec!["C02-modify-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Dirty,
        expect: Expectation::Failure {
            phase: "execute",
            contains: "not clean",
            target_unchanged: true,
        },
        accept_review: false,
    }
}

fn case_i03() -> CaseSpec {
    CaseSpec {
        id: "I03-GG",
        commits: vec!["C01-add-text"],
        mappings: bad_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Failure {
            phase: "preview",
            contains: "mapping",
            target_unchanged: true,
        },
        accept_review: false,
    }
}

fn case_i04() -> CaseSpec {
    CaseSpec {
        id: "I04-GG",
        commits: vec!["C07-same-file-step1", "C08-same-file-step2"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: changed(&[("src/chain.txt", STEP2_CHAIN)]),
            deleted: vec![],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 2,
        },
        accept_review: false,
    }
}

fn case_i05() -> CaseSpec {
    CaseSpec {
        id: "I05-GG",
        commits: vec!["C09-add-then-delete-a", "C10-add-then-delete-b"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: BTreeMap::new(),
            deleted: vec![],
            absent: vec!["tmp/transient.txt"],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 2,
        },
        accept_review: false,
    }
}

fn case_i06() -> CaseSpec {
    CaseSpec {
        id: "I06-GG",
        commits: vec!["C03-delete-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: BTreeMap::new(),
            deleted: vec!["src/delete_me.txt"],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 1,
        },
        accept_review: true,
    }
}

fn case_i07() -> CaseSpec {
    CaseSpec {
        id: "I07-GG",
        commits: vec!["C01-add-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::OccupiedNewFile,
        expect: Expectation::Failure {
            phase: "execute",
            contains: "blocked",
            target_unchanged: true,
        },
        accept_review: false,
    }
}

fn case_i08() -> CaseSpec {
    CaseSpec {
        id: "I08-GG",
        commits: vec!["C02-modify-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::FailingCommitHook,
        expect: Expectation::Failure {
            phase: "execute",
            contains: "commit",
            target_unchanged: true,
        },
        accept_review: false,
    }
}

fn case_d01() -> CaseSpec {
    CaseSpec {
        id: "D01-GG",
        commits: vec!["C01-add-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: changed(&[("src/new_file.txt", NEW_FILE)]),
            deleted: vec![],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 1,
        },
        accept_review: false,
    }
}

fn case_d03() -> CaseSpec {
    CaseSpec {
        id: "D03-GG",
        commits: vec!["C03-delete-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: BTreeMap::new(),
            deleted: vec!["src/delete_me.txt"],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 1,
        },
        accept_review: true,
    }
}

fn case_f05() -> CaseSpec {
    CaseSpec {
        id: "F05-GG",
        commits: vec!["C01-add-text", "C02-modify-text", "C03-delete-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Success {
            changed: changed(&[
                ("src/new_file.txt", NEW_FILE),
                ("src/existing.txt", MOD_EXISTING),
            ]),
            deleted: vec!["src/delete_me.txt"],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt"],
            commit_count: 3,
        },
        accept_review: true,
    }
}

fn case_e07() -> CaseSpec {
    CaseSpec {
        id: "E07-GG",
        commits: vec!["C03-delete-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::Clean,
        expect: Expectation::Failure {
            phase: "execute",
            contains: "review",
            target_unchanged: true,
        },
        accept_review: false,
    }
}

fn case_f10() -> CaseSpec {
    CaseSpec {
        id: "F10-GG",
        commits: vec!["C02-modify-text"],
        mappings: default_mapping(),
        target_setup: TargetSetup::AlreadyHasModify,
        expect: Expectation::Success {
            changed: BTreeMap::new(),
            deleted: vec![],
            absent: vec![],
            unchanged: vec!["README.md", "src/target_only.txt", "src/existing.txt"],
            commit_count: 1,
        },
        accept_review: false,
    }
}

fn case_to_json(case: &CaseSpec) -> serde_json::Value {
    serde_json::json!({
        "id": case.id,
        "sourceType": "git",
        "targetType": "git",
        "sourceCommits": case.commits,
        "pathMappings": case.mappings.iter().map(|m| {
            serde_json::json!({"from": m.from, "to": m.to})
        }).collect::<Vec<_>>(),
        "migrationMode": "incremental_first",
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
