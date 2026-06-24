use crate::error::{AppError, Result};

use super::log_parser::parse_log_xml;

#[derive(Debug, Clone, Default)]
pub struct SvnCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Decode SVN CLI stdout. On Windows, SVN often uses system ANSI (e.g. GBK) for log messages.
fn decode_svn_output(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    // Typical on zh-CN Windows when commit messages contain non-ASCII.
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if !had_errors {
        return decoded.into_owned();
    }

    // Last resort: preserve bytes, avoid hard failure on mixed encodings.
    String::from_utf8_lossy(bytes).into_owned()
}

/// Run an SVN subcommand and return stdout on success.
pub fn run_svn(url: &str, creds: &SvnCredentials, args: &[&str]) -> Result<String> {
    let mut cmd = crate::process::command("svn");
    // --non-interactive: do not prompt; fail if credentials are missing.
    // Avoid --no-auth-prompt: not supported on older SVN builds (e.g. some Windows installs).
    cmd.arg("--non-interactive");

    if let Some(user) = &creds.username {
        cmd.arg("--username").arg(user);
    }
    if let Some(pass) = &creds.password {
        cmd.arg("--password").arg(pass);
    }

    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg(url);

    let output = cmd
        .output()
        .map_err(|e| AppError::Vcs(format!("failed to spawn svn: {e}")))?;

    if output.status.success() {
        return Ok(decode_svn_output(&output.stdout));
    }

    let stderr = decode_svn_output(&output.stderr);
    let stdout = decode_svn_output(&output.stdout);
    Err(AppError::Vcs(format!(
        "svn {} failed (exit {:?}): {stderr}{stdout}",
        args.first().copied().unwrap_or(""),
        output.status.code(),
    )))
}

pub fn svn_log_xml(url: &str, creds: &SvnCredentials, limit: usize) -> Result<String> {
    run_svn(
        url,
        creds,
        &["log", "--xml", "-v", "-l", &limit.to_string()],
    )
}

/// Paginated log: return up to `limit` entries older than `before_revision` (exclusive).
/// `before_revision = None` fetches from HEAD.
/// Pass the repository URL (from `svn info`), not a working-copy path — WC paths default to BASE:1.
pub fn svn_log_xml_paged(
    url: &str,
    creds: &SvnCredentials,
    limit: usize,
    before_revision: Option<u64>,
) -> Result<Vec<crate::model::ReplayUnitMeta>> {
    let limit_s = limit.to_string();
    let xml = match before_revision {
        None => run_svn(url, creds, &["log", "--xml", "-v", "-l", &limit_s])?,
        Some(rev) if rev <= 1 => return Ok(Vec::new()),
        Some(rev) => {
            let range = format!("{}:1", rev - 1);
            run_svn(
                url,
                creds,
                &["log", "--xml", "-v", "-l", &limit_s, "-r", &range],
            )?
        }
    };
    parse_log_xml(&xml)
}

pub fn svn_diff_revision(url: &str, creds: &SvnCredentials, revision: u64) -> Result<String> {
    run_svn(url, creds, &["diff", "-c", &revision.to_string()])
}

pub fn svn_log_revision_xml(url: &str, creds: &SvnCredentials, revision: u64) -> Result<String> {
    run_svn(
        url,
        creds,
        &["log", "--xml", "-r", &revision.to_string(), "-l", "1"],
    )
}

pub fn svn_cat_file(creds: &SvnCredentials, revision: u64, file_path: &str) -> Result<String> {
    run_svn(
        file_path,
        creds,
        &["cat", "-r", &revision.to_string()],
    )
}

pub fn svn_info_xml(wc_path: &str) -> Result<String> {
    run_svn(wc_path, &SvnCredentials::default(), &["info", "--xml"])
}

pub fn probe_svn_wc(wc_path: &str) -> Result<super::info_parser::SvnWcInfo> {
    let xml = svn_info_xml(wc_path)?;
    super::info_parser::parse_info_xml(&xml)
}

/// Detect the last segment of the working copy's SVN relative URL (e.g. `hk`, `trunk`).
pub fn detect_svn_branch(wc_path: &str) -> Option<String> {
    probe_svn_wc(wc_path).ok().map(|info| info.branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_svn_fails_when_svn_missing() {
        let result = crate::process::command("svn_nonexistent_binary_42").output();
        assert!(result.is_err());
    }

    #[test]
    fn decodes_valid_utf8() {
        assert_eq!(decode_svn_output(b"<?xml version=\"1.0\"?>"), "<?xml version=\"1.0\"?>");
    }

    #[test]
    fn decodes_gbk_when_not_utf8() {
        // GBK encoding of two Chinese characters commonly used in tests.
        let gbk = [0xB2, 0xE2, 0xCA, 0xD4];
        let text = decode_svn_output(&gbk);
        assert_eq!(text, "测试");
    }
}
