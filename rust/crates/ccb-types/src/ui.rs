pub mod text {
    pub const STATUS_CANDIDATE: &str = "candidate only";
    pub const STATUS_PENDING_AUTH: &str = "pending authorization";
    pub const STATUS_PENDING_PLATFORM: &str = "pending platform confirmation";
    pub const STATUS_BLOCKED: &str = "blocked";
    pub const STATUS_READ_ONLY: &str = "read-only";
    pub const STATUS_NOT_OPEN: &str = "not open";
}

pub mod i18n {
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Language {
        English,
        Chinese,
    }

    impl Language {
        pub fn as_str(&self) -> &'static str {
            match self {
                Language::English => "en",
                Language::Chinese => "zh",
            }
        }
    }

    impl FromStr for Language {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "en" | "english" => Ok(Language::English),
                "zh" | "cn" | "chinese" => Ok(Language::Chinese),
                _ => Err(format!("unsupported language: {s}")),
            }
        }
    }

    static CURRENT_LANG: Mutex<Option<Language>> = Mutex::new(None);

    fn messages() -> &'static HashMap<Language, HashMap<&'static str, &'static str>> {
        static MESSAGES: OnceLock<HashMap<Language, HashMap<&'static str, &'static str>>> =
            OnceLock::new();
        MESSAGES.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(Language::English, english_messages());
            map.insert(Language::Chinese, chinese_messages());
            map
        })
    }

    fn english_messages() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("no_terminal_backend", "No tmux backend detected"),
            ("solutions", "Solutions:"),
            (
                "install_tmux",
                "Install tmux: https://github.com/tmux/tmux/wiki/Installing",
            ),
            (
                "tmux_installed_not_inside",
                "tmux is installed, but you're not inside a tmux session (run `tmux` first, then run `ccb` inside tmux)",
            ),
            ("tmux_not_installed", "tmux is not installed"),
            ("creating_tmux_session", "Creating tmux session: {session}"),
            ("attaching_to_tmux", "Attaching to tmux session: {session}"),
            (
                "starting_backend",
                "Starting {provider} backend ({terminal})...",
            ),
            ("started_backend", "{provider} started ({terminal}: {pane_id})"),
            ("unknown_provider", "Unknown provider: {provider}"),
            (
                "resuming_session",
                "Resuming {provider} session: {session_id}...",
            ),
            (
                "no_history_fresh",
                "No {provider} history found, starting fresh",
            ),
            ("warmup", "Warmup: {script}"),
            ("warmup_failed", "Warmup failed: {provider}"),
            ("starting_claude", "Starting Claude..."),
            (
                "resuming_claude",
                "Resuming Claude session: {session_id}...",
            ),
            (
                "no_claude_session",
                "No local Claude session found, starting fresh",
            ),
            ("session_id", "Session ID: {session_id}"),
            ("runtime_dir", "Runtime dir: {runtime_dir}"),
            ("active_backends", "Active backends: {backends}"),
            ("available_commands", "Available commands:"),
            ("codex_commands", "ask/ping/pend - Codex communication"),
            ("gemini_commands", "ask/ping/pend - Gemini communication"),
            ("droid_commands", "ask/ping/pend - Droid communication"),
            ("executing", "Executing: {cmd}"),
            ("user_interrupted", "User interrupted"),
            ("cleaning_up", "Cleaning up session resources..."),
            ("cleanup_complete", "Cleanup complete"),
            ("banner_title", "Claude Code Bridge {version}"),
            ("banner_date", "{date}"),
            ("banner_backends", "Backends: {backends}"),
            ("cannot_write_session", "Cannot write {filename}: {reason}"),
            ("fix_hint", "Fix: {fix}"),
            ("error", "Error"),
            ("execution_failed", "Execution failed: {error}"),
            ("import_failed", "Import failed: {error}"),
            ("module_import_failed", "Module import failed: {error}"),
            (
                "connectivity_test_failed",
                "{provider} connectivity test failed: {error}",
            ),
            ("no_reply_available", "No {provider} reply available"),
            ("usage", "Usage: {cmd}"),
            ("sending_to", "Sending question to {provider}..."),
            (
                "waiting_for_reply",
                "Waiting for {provider} reply (no timeout, Ctrl-C to interrupt)...",
            ),
            ("reply_from", "{provider} reply:"),
            ("timeout_no_reply", "Timeout: no reply from {provider}"),
            ("session_not_found", "No active {provider} session found"),
            ("install_complete", "Installation complete"),
            ("uninstall_complete", "Uninstall complete"),
            ("python_version_old", "Python version too old: {version}"),
            ("requires_python", "Requires Python 3.10+"),
            ("missing_dependency", "Missing dependency: {dep}"),
            ("detected_env", "Detected {env} environment"),
            ("confirm_continue", "Confirm continue? (y/N)"),
            ("cancelled", "Cancelled"),
        ])
    }

    fn chinese_messages() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("no_terminal_backend", "未检测到 tmux 后端"),
            ("solutions", "解决方案："),
            (
                "install_tmux",
                "安装 tmux: https://github.com/tmux/tmux/wiki/Installing",
            ),
            (
                "tmux_installed_not_inside",
                "已安装 tmux，但当前不在 tmux 会话中（请先运行 `tmux`，再在 tmux 内执行 `ccb`）",
            ),
            ("tmux_not_installed", "tmux 未安装"),
            ("creating_tmux_session", "正在创建 tmux 会话: {session}"),
            ("attaching_to_tmux", "正在连接到 tmux 会话: {session}"),
            (
                "starting_backend",
                "正在启动 {provider} 后端 ({terminal})...",
            ),
            (
                "started_backend",
                "{provider} 已启动 ({terminal}: {pane_id})",
            ),
            ("unknown_provider", "未知提供者: {provider}"),
            (
                "resuming_session",
                "正在恢复 {provider} 会话: {session_id}...",
            ),
            ("no_history_fresh", "未找到 {provider} 历史记录，全新启动"),
            ("warmup", "预热: {script}"),
            ("warmup_failed", "预热失败: {provider}"),
            ("starting_claude", "正在启动 Claude..."),
            ("resuming_claude", "正在恢复 Claude 会话: {session_id}..."),
            ("no_claude_session", "未找到本地 Claude 会话，全新启动"),
            ("session_id", "会话 ID: {session_id}"),
            ("runtime_dir", "运行目录: {runtime_dir}"),
            ("active_backends", "活动后端: {backends}"),
            ("available_commands", "可用命令："),
            ("codex_commands", "ask/ping/pend - Codex 通信"),
            ("gemini_commands", "ask/ping/pend - Gemini 通信"),
            ("droid_commands", "ask/ping/pend - Droid 通信"),
            ("executing", "执行: {cmd}"),
            ("user_interrupted", "用户中断"),
            ("cleaning_up", "正在清理会话资源..."),
            ("cleanup_complete", "清理完成"),
            ("banner_title", "Claude Code Bridge {version}"),
            ("banner_date", "{date}"),
            ("banner_backends", "后端: {backends}"),
            ("cannot_write_session", "无法写入 {filename}: {reason}"),
            ("fix_hint", "修复: {fix}"),
            ("error", "错误"),
            ("execution_failed", "执行失败: {error}"),
            ("import_failed", "导入失败: {error}"),
            ("module_import_failed", "模块导入失败: {error}"),
            (
                "connectivity_test_failed",
                "{provider} 连通性测试失败: {error}",
            ),
            ("no_reply_available", "暂无 {provider} 回复"),
            ("usage", "用法: {cmd}"),
            ("sending_to", "正在发送问题到 {provider}..."),
            (
                "waiting_for_reply",
                "等待 {provider} 回复 (无超时，Ctrl-C 中断)...",
            ),
            ("reply_from", "{provider} 回复:"),
            ("timeout_no_reply", "超时: 未收到 {provider} 回复"),
            ("session_not_found", "未找到活动的 {provider} 会话"),
            ("install_complete", "安装完成"),
            ("uninstall_complete", "卸载完成"),
            ("python_version_old", "Python 版本过旧: {version}"),
            ("requires_python", "需要 Python 3.10+"),
            ("missing_dependency", "缺少依赖: {dep}"),
            ("detected_env", "检测到 {env} 环境"),
            ("confirm_continue", "确认继续？(y/N)"),
            ("cancelled", "已取消"),
        ])
    }

    pub fn detect_language() -> Language {
        if let Ok(ccb_lang) = std::env::var("CCB_LANG") {
            let ccb_lang = ccb_lang.to_lowercase();
            if ccb_lang == "zh" || ccb_lang == "cn" || ccb_lang == "chinese" {
                return Language::Chinese;
            }
            if ccb_lang == "en" || ccb_lang == "english" {
                return Language::English;
            }
        }

        for var in ["LANG", "LC_ALL", "LC_MESSAGES"] {
            if let Ok(value) = std::env::var(var) {
                let value = value.to_lowercase();
                if value.starts_with("zh") || value.contains("chinese") {
                    return Language::Chinese;
                }
            }
        }

        if let Some(locale) = sys_locale::get_locale() {
            let locale = locale.to_string().to_lowercase();
            if locale.starts_with("zh") || locale.contains("chinese") {
                return Language::Chinese;
            }
        }

        Language::English
    }

    pub fn get_lang() -> Language {
        let mut current = CURRENT_LANG.lock().unwrap();
        *current.get_or_insert_with(detect_language)
    }

    pub fn set_lang(lang: Language) {
        let mut current = CURRENT_LANG.lock().unwrap();
        *current = Some(lang);
    }

    pub fn t(key: &str, args: &[(&str, &str)]) -> String {
        let lang = get_lang();
        let all_messages = messages();
        let lang_messages = all_messages
            .get(&lang)
            .unwrap_or_else(|| all_messages.get(&Language::English).unwrap());
        let msg = lang_messages.get(key).copied().unwrap_or_else(|| {
            all_messages
                .get(&Language::English)
                .and_then(|m| m.get(key).copied())
                .unwrap_or(key)
        });
        format_message(msg, args)
    }

    fn format_message(template: &str, args: &[(&str, &str)]) -> String {
        let mut result = template.to_string();
        for (name, value) in args {
            result = result.replace(&format!("{{{name}}}"), value);
        }
        result
    }
}
