use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use tauri::{AppHandle, Manager};

use super::crypto::{decrypt_secret, encrypt_secret};
use super::models::{
    default_svn_mappings, EditorRecord, MigrationRecord, RepoInput, RepoPairMappingInput,
    RepoPairMappingRecord, RepoPairMappingView, RepoRecord, RepoType, RepoView, EDITOR_PRESETS,
};
use crate::error::{AppError, Result};
use crate::mapper::PathMapping;

pub struct DbState(pub Mutex<Connection>);

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Other(anyhow::anyhow!("db: {e}"))
}

pub fn init_db(app: &AppHandle) -> Result<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
    std::fs::create_dir_all(&dir)?;
    let db_path: PathBuf = dir.join("relay.db");
    let conn = Connection::open(db_path).map_err(db_err)?;
    run_migrations(&conn)?;
    seed_editors(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS repos (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS editors (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS migrations (
            id TEXT PRIMARY KEY,
            completed_at TEXT NOT NULL,
            data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_state (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS repo_pair_mappings (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            data TEXT NOT NULL,
            PRIMARY KEY (source_id, target_id)
        );
        CREATE TABLE IF NOT EXISTS relayed_commits (
            source_id TEXT NOT NULL,
            commit_ref TEXT NOT NULL,
            relayed_at TEXT NOT NULL,
            PRIMARY KEY (source_id, commit_ref)
        );
        ",
    )
    .map_err(db_err)?;
    backfill_relayed_commits(conn)?;
    Ok(())
}

fn repo_path_key(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn backfill_relayed_commits(conn: &Connection) -> Result<()> {
    let repos = list_repos(conn)?;
    let migrations = list_migrations(conn)?;
    for m in migrations {
        if m.status != "success" {
            continue;
        }
        let path_key = repo_path_key(&m.source.path);
        let Some(source_id) = repos
            .iter()
            .find(|r| repo_path_key(&r.path) == path_key)
            .map(|r| r.id.as_str())
        else {
            continue;
        };
        for c in &m.commits {
            conn.execute(
                "INSERT OR IGNORE INTO relayed_commits (source_id, commit_ref, relayed_at) VALUES (?1, ?2, ?3)",
                params![source_id, c.id, m.completed_at],
            )
            .map_err(db_err)?;
        }
    }
    Ok(())
}

pub fn record_relayed_commits(
    conn: &Connection,
    source_id: &str,
    commit_refs: &[String],
    relayed_at: &str,
) -> Result<()> {
    for commit_ref in commit_refs {
        conn.execute(
            "INSERT OR REPLACE INTO relayed_commits (source_id, commit_ref, relayed_at) VALUES (?1, ?2, ?3)",
            params![source_id, commit_ref, relayed_at],
        )
        .map_err(db_err)?;
    }
    Ok(())
}

pub fn list_relayed_commits(conn: &Connection, source_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT commit_ref FROM relayed_commits WHERE source_id = ?1")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![source_id], |row| row.get(0))
        .map_err(db_err)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(db_err)?);
    }
    Ok(out)
}


fn seed_editors(conn: &Connection) -> Result<()> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM editors", [], |r| r.get(0))
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    if count > 0 {
        return Ok(());
    }
    for (i, (id, name, kind, exe)) in EDITOR_PRESETS.iter().enumerate() {
        let editor = EditorRecord {
            id: (*id).into(),
            name: (*name).into(),
            exe: (*exe).into(),
            kind: (*kind).into(),
            custom: false,
        };
        let json = serde_json::to_string(&editor)
            .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        conn.execute(
            "INSERT INTO editors (id, data, is_default) VALUES (?1, ?2, ?3)",
            params![id, json, i == 0],
        )
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    }
    Ok(())
}

fn to_view(repo: &RepoRecord) -> RepoView {
    RepoView {
        id: repo.id.clone(),
        name: repo.name.clone(),
        path: repo.path.clone(),
        repo_type: repo.repo_type.clone(),
        branch: repo.branch.clone(),
        last_used: repo.last_used.clone(),
        svn_user: repo.svn_user.clone(),
        has_svn_pass: repo
            .svn_pass_encrypted
            .as_ref()
            .is_some_and(|s| !s.is_empty()),
        path_mappings: repo.path_mappings.clone(),
    }
}

pub fn list_repos(conn: &Connection) -> Result<Vec<RepoView>> {
    let mut stmt = conn
        .prepare("SELECT data FROM repos ORDER BY json_extract(data, '$.name')")
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let rows = stmt
        .query_map([], |row| {
            let data: String = row.get(0)?;
            Ok(data)
        })
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let mut repos = Vec::new();
    for row in rows {
        let data = row.map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        let repo: RepoRecord = serde_json::from_str(&data)
            .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        repos.push(to_view(&repo));
    }
    Ok(repos)
}

pub fn get_repo(conn: &Connection, id: &str) -> Result<Option<RepoRecord>> {
    let mut stmt = conn
        .prepare("SELECT data FROM repos WHERE id = ?1")
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let mut rows = stmt
        .query(params![id])
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    match rows.next() {
        Ok(Some(row)) => {
            let data: String = row.get(0).map_err(db_err)?;
            let repo: RepoRecord = serde_json::from_str(&data)
                .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
            Ok(Some(repo))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(db_err(e)),
    }
}

pub fn get_repo_decrypted(conn: &Connection, id: &str) -> Result<Option<RepoRecord>> {
    get_repo(conn, id)
}

pub fn decrypt_repo_pass(repo: &RepoRecord) -> Result<Option<String>> {
    match &repo.svn_pass_encrypted {
        Some(enc) if !enc.is_empty() => Ok(Some(decrypt_secret(enc)?)),
        _ => Ok(None),
    }
}

pub fn save_repo(conn: &Connection, input: RepoInput) -> Result<RepoView> {
    if input.name.trim().is_empty() || input.path.trim().is_empty() {
        return Err(AppError::Validation("name and path are required".into()));
    }

    let id = input
        .id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let existing = get_repo(conn, &id)?;
    let svn_pass_encrypted = if let Some(pass) = input.svn_pass.filter(|p| !p.is_empty()) {
        Some(encrypt_secret(&pass)?)
    } else {
        existing
            .as_ref()
            .and_then(|r| r.svn_pass_encrypted.clone())
    };

    let mut branch = if input.branch.trim().is_empty() {
        "main".into()
    } else {
        input.branch.trim().to_string()
    };
    if matches!(input.repo_type, RepoType::Svn) && branch == "trunk" {
        if let Some(detected) = crate::vcs::svn::detect_svn_branch(input.path.trim()) {
            branch = detected;
        }
    }

    let path_mappings = if input.path_mappings.is_empty() {
        match input.repo_type {
            RepoType::Svn => default_svn_mappings(&branch),
            RepoType::Git => vec![],
        }
    } else {
        input.path_mappings
    };

    let record = RepoRecord {
        id: id.clone(),
        name: input.name.trim().to_string(),
        path: input.path.trim().to_string(),
        repo_type: input.repo_type,
        branch,
        last_used: existing.and_then(|r| r.last_used),
        svn_user: input.svn_user.filter(|s| !s.is_empty()),
        svn_pass_encrypted,
        path_mappings,
    };

    let json = serde_json::to_string(&record)
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "INSERT INTO repos (id, data) VALUES (?1, ?2)
         ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        params![id, json],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(to_view(&record))
}

pub fn delete_repo(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM repos WHERE id = ?1", params![id])
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "DELETE FROM repo_pair_mappings WHERE source_id = ?1 OR target_id = ?1",
        params![id],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(())
}

pub fn touch_repo(conn: &Connection, id: &str) -> Result<()> {
    if let Some(mut repo) = get_repo(conn, id)? {
        repo.last_used = Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
        let json = serde_json::to_string(&repo)
            .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        conn.execute(
            "UPDATE repos SET data = ?2 WHERE id = ?1",
            params![id, json],
        )
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    }
    Ok(())
}

pub fn list_editors(conn: &Connection) -> Result<Vec<EditorRecord>> {
    let mut stmt = conn
        .prepare("SELECT data FROM editors ORDER BY rowid")
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let mut editors = Vec::new();
    for row in rows {
        let data = row.map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        let mut editor: EditorRecord =
            serde_json::from_str(&data).map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        editor.exe = super::editor_paths::resolve_editor_exe(&editor.kind, &editor.exe);
        editors.push(editor);
    }
    Ok(editors)
}

pub fn get_default_editor_id(conn: &Connection) -> Result<String> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM editors WHERE is_default = 1 LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = id {
        return Ok(id);
    }
    let id: String = conn
        .query_row("SELECT id FROM editors ORDER BY rowid LIMIT 1", [], |r| {
            r.get(0)
        })
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(id)
}

pub fn set_default_editor(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE editors SET is_default = 0", [])
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "UPDATE editors SET is_default = 1 WHERE id = ?1",
        params![id],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(())
}

pub fn save_editor(conn: &Connection, editor: EditorRecord) -> Result<EditorRecord> {
    if editor.name.trim().is_empty() || editor.exe.trim().is_empty() {
        return Err(AppError::Validation("editor name and exe are required".into()));
    }
    let json = serde_json::to_string(&editor)
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "INSERT INTO editors (id, data, is_default) VALUES (?1, ?2, 0)
         ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        params![editor.id, json],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(editor)
}

pub fn delete_editor(conn: &Connection, id: &str) -> Result<()> {
    let custom: bool = conn
        .query_row(
            "SELECT json_extract(data, '$.custom') FROM editors WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    if !custom {
        return Err(AppError::Validation("cannot remove preset editor".into()));
    }
    let was_default: bool = conn
        .query_row(
            "SELECT is_default FROM editors WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute("DELETE FROM editors WHERE id = ?1", params![id])
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    if was_default {
        if let Ok(first) = conn.query_row(
            "SELECT id FROM editors ORDER BY rowid LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        ) {
            set_default_editor(conn, &first)?;
        }
    }
    Ok(())
}

pub fn list_migrations(conn: &Connection) -> Result<Vec<MigrationRecord>> {
    let mut stmt = conn
        .prepare("SELECT data FROM migrations ORDER BY completed_at DESC")
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
        out.push(
            serde_json::from_str(&data).map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?,
        );
    }
    Ok(out)
}

pub fn get_migration(conn: &Connection, id: &str) -> Result<Option<MigrationRecord>> {
    let mut stmt = conn
        .prepare("SELECT data FROM migrations WHERE id = ?1")
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    let mut rows = stmt
        .query(params![id])
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    match rows.next() {
        Ok(Some(row)) => {
            let data: String = row.get(0).map_err(db_err)?;
            Ok(Some(
                serde_json::from_str(&data).map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?,
            ))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(db_err(e)),
    }
}

pub fn save_migration(conn: &Connection, record: MigrationRecord) -> Result<MigrationRecord> {
    let json = serde_json::to_string(&record)
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "INSERT INTO migrations (id, completed_at, data) VALUES (?1, ?2, ?3)",
        params![record.id, record.completed_at, json],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(record)
}

pub fn repo_path_mappings(repo: &RepoRecord) -> Vec<PathMapping> {
    if repo.path_mappings.is_empty() {
        match repo.repo_type {
            RepoType::Svn => default_svn_mappings(&repo.branch),
            RepoType::Git => vec![],
        }
    } else {
        repo.path_mappings.clone()
    }
}

pub fn get_repo_pair_mapping(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
) -> Result<Option<RepoPairMappingView>> {
    let data: Option<String> = conn
        .query_row(
            "SELECT data FROM repo_pair_mappings WHERE source_id = ?1 AND target_id = ?2",
            params![source_id, target_id],
            |r| r.get(0),
        )
        .ok();
    match data {
        Some(json) => {
            let record: RepoPairMappingRecord = serde_json::from_str(&json)
                .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
            Ok(Some(RepoPairMappingView {
                path_mappings: record.path_mappings,
                custom_mapping: record.custom_mapping,
            }))
        }
        None => Ok(None),
    }
}

pub fn save_repo_pair_mapping(
    conn: &Connection,
    input: RepoPairMappingInput,
) -> Result<RepoPairMappingView> {
    let record = RepoPairMappingRecord {
        source_id: input.source_id.clone(),
        target_id: input.target_id.clone(),
        path_mappings: input.path_mappings.clone(),
        custom_mapping: input.custom_mapping,
    };
    let json = serde_json::to_string(&record)
        .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    conn.execute(
        "INSERT INTO repo_pair_mappings (source_id, target_id, data) VALUES (?1, ?2, ?3)
         ON CONFLICT(source_id, target_id) DO UPDATE SET data = excluded.data",
        params![input.source_id, input.target_id, json],
    )
    .map_err(|e| AppError::Other(anyhow::anyhow!("{e}")))?;
    Ok(RepoPairMappingView {
        path_mappings: record.path_mappings,
        custom_mapping: record.custom_mapping,
    })
}
