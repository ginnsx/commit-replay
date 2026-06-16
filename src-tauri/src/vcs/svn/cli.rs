use std::process::Command;

use crate::error::{AppError, Result};

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
    let mut cmd = Command::new("svn");
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
        &["log", "--xml", "-l", &limit.to_string()],
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_svn_fails_when_svn_missing() {
        let result = Command::new("svn_nonexistent_binary_42").output();
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
