use crate::model::ReplayUnitMeta;

pub fn replay_message(meta: &ReplayUnitMeta) -> String {
    format!("{}: {}", relay_marker(), commit_title(&meta.message))
}

pub fn finalize_message() -> String {
    format!("{}: 已完成冲突处理", relay_marker())
}

pub fn squash_message(user_message: &str, metas: &[ReplayUnitMeta]) -> String {
    let mut msg = format!("{}\n\n{}。\n\n原提交:", user_message.trim(), relay_marker());
    for meta in metas {
        msg.push_str(&format!(
            "\n- {} {}",
            short_source_ref(&meta.source_ref),
            commit_title(&meta.message)
        ));
    }
    msg
}

pub fn relay_marker() -> String {
    format!("使用 Relay v{} 合并", app_version())
}

pub fn app_version() -> &'static str {
    env!("RELAY_APP_VERSION")
}

fn commit_title(message: &str) -> &str {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("无标题提交")
}

fn short_source_ref(source_ref: &str) -> String {
    if let Some(sha) = source_ref.strip_prefix("git:") {
        return format!("git:{}", sha.chars().take(7).collect::<String>());
    }
    source_ref.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(source_ref: &str, message: &str) -> ReplayUnitMeta {
        ReplayUnitMeta {
            source_ref: source_ref.into(),
            author: "tester".into(),
            date: "2026-07-03".into(),
            message: message.into(),
            changed_paths_count: 1,
        }
    }

    #[test]
    fn replay_message_contains_relay_version_and_commit_title() {
        let msg = replay_message(&meta("git:abcdef123456", "fix source bug\n\nbody"));

        assert!(msg.contains(&relay_marker()));
        assert!(msg.contains("fix source bug"));
    }

    #[test]
    fn finalize_message_contains_relay_version_and_conflict_hint() {
        let msg = finalize_message();

        assert!(msg.contains(&relay_marker()));
        assert!(msg.contains("已完成冲突处理"));
    }

    #[test]
    fn squash_message_keeps_user_subject_and_lists_original_commits() {
        let metas = vec![
            meta("git:abcdef123456", "add feature"),
            meta("svn:42", "修复缺陷\n\nbody"),
        ];
        let msg = squash_message("relay regression squash", &metas);

        assert!(msg.starts_with("relay regression squash\n\n"));
        assert!(msg.contains(&relay_marker()));
        assert!(msg.contains("原提交:"));
        assert!(msg.contains("- git:abcdef1 add feature"));
        assert!(msg.contains("- svn:42 修复缺陷"));
    }
}
