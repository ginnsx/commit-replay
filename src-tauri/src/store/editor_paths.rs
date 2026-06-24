use std::path::{Path, PathBuf};

pub fn resolve_editor_exe(kind: &str, exe: &str) -> String {
    let trimmed = exe.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let path = Path::new(trimmed);
    if path.is_absolute() && path.exists() {
        return trimmed.to_string();
    }

    resolve_by_kind(kind)
        .or_else(|| resolve_in_path(trimmed))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| trimmed.to_string())
}

fn resolve_in_path(name: &str) -> Option<PathBuf> {
    let output = crate::process::command("where").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?;
    let path = PathBuf::from(first);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn resolve_by_kind(kind: &str) -> Option<PathBuf> {
    match kind {
        "vscode" => first_existing(&vscode_candidates()),
        "cursor" => first_existing(&cursor_candidates()),
        "visualstudio" => find_vs_devenv(),
        "idea" => find_jetbrains_bin("IntelliJ IDEA", "idea64.exe"),
        "pycharm" => find_jetbrains_bin("PyCharm", "pycharm64.exe"),
        "gitbash" => first_existing(&gitbash_candidates()),
        "explorer" => first_existing(&explorer_candidates()),
        "terminal" => resolve_in_path("wt.exe"),
        _ => None,
    }
}

fn first_existing(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| p.exists()).cloned()
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var(var).ok().map(PathBuf::from)
}

fn vscode_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(local) = env_path("LOCALAPPDATA") {
        paths.push(local.join("Programs/Microsoft VS Code/Code.exe"));
    }
    if let Some(pf) = env_path("ProgramFiles") {
        paths.push(pf.join("Microsoft VS Code/Code.exe"));
    }
    if let Some(pf86) = env_path("ProgramFiles(x86)") {
        paths.push(pf86.join("Microsoft VS Code/Code.exe"));
    }
    paths
}

fn cursor_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(local) = env_path("LOCALAPPDATA") {
        paths.push(local.join("Programs/cursor/Cursor.exe"));
        paths.push(local.join("Programs/Cursor/Cursor.exe"));
    }
    paths
}

fn gitbash_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(pf) = env_path("ProgramFiles") {
        paths.push(pf.join("Git/git-bash.exe"));
    }
    if let Some(pf86) = env_path("ProgramFiles(x86)") {
        paths.push(pf86.join("Git/git-bash.exe"));
    }
    paths
}

fn explorer_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(windir) = env_path("WINDIR") {
        paths.push(windir.join("explorer.exe"));
    }
    paths.push(PathBuf::from(r"C:\Windows\explorer.exe"));
    paths
}

fn find_vs_devenv() -> Option<PathBuf> {
    let roots = [env_path("ProgramFiles"), env_path("ProgramFiles(x86)")]
        .into_iter()
        .flatten()
        .map(|p| p.join("Microsoft Visual Studio"));
    for root in roots {
        let found = find_file_under(&root, "devenv.exe", 4);
        if found.is_some() {
            return found;
        }
    }
    None
}

fn find_jetbrains_bin(product_prefix: &str, bin_name: &str) -> Option<PathBuf> {
    let roots = [env_path("ProgramFiles"), env_path("ProgramFiles(x86)")]
        .into_iter()
        .flatten()
        .map(|p| p.join("JetBrains"));
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(product_prefix) {
                continue;
            }
            let candidate = entry.path().join("bin").join(bin_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_file_under(root: &Path, file_name: &str, max_depth: u32) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|n| n == file_name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_under(&path, file_name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_absolute_existing_path() {
        let exe = std::env::current_exe().unwrap();
        let resolved = resolve_editor_exe("custom", &exe.to_string_lossy());
        assert_eq!(resolved, exe.to_string_lossy());
    }

    #[test]
    fn keeps_unknown_short_name_when_not_found() {
        assert_eq!(
            resolve_editor_exe("custom", "nonexistent-editor-42.exe"),
            "nonexistent-editor-42.exe"
        );
    }
}
