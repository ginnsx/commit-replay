//! Integration tests against a real SVN repository.
//!
//! Run (optional):
//!   set COPY_DIFF_SVN_URL=https://svn.example.com/repo
//!   set COPY_DIFF_SVN_USER=user
//!   set COPY_DIFF_SVN_PASS=pass
//!   cargo test --test svn_integration -- --ignored

use copy_diff_lib::vcs::svn_reader::SvnReader;
use copy_diff_lib::vcs::VcsReader;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

fn reader_from_env() -> Option<SvnReader> {
    let url = env("COPY_DIFF_SVN_URL")?;
    Some(SvnReader {
        url,
        username: env("COPY_DIFF_SVN_USER"),
        password: env("COPY_DIFF_SVN_PASS"),
    })
}

#[test]
#[ignore = "requires COPY_DIFF_SVN_URL and network access to SVN"]
fn list_recent_from_live_repo() {
    let reader = reader_from_env().expect("set COPY_DIFF_SVN_URL");
    let entries = reader.list_recent(5).expect("svn log");
    assert!(!entries.is_empty(), "expected at least one log entry");
    assert!(entries[0].source_ref.starts_with("svn:"));
}

#[test]
#[ignore = "requires COPY_DIFF_SVN_URL and network access to SVN"]
fn load_changeset_from_live_repo() {
    let reader = reader_from_env().expect("set COPY_DIFF_SVN_URL");
    let entries = reader.list_recent(1).expect("svn log");
    let source_ref = entries[0].source_ref.clone();
    let changeset = reader.load_changeset(&source_ref).expect("load changeset");
    assert_eq!(changeset.meta.source_ref, source_ref);
}
