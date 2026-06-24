//! Mirrors Python `lib/cli/kill_runtime/zombies.py`.

use std::io::{self, Write};
use std::process::Command;

/// Providers that appear in CCB tmux session names with an embedded parent PID.
const ZOMBIE_PROVIDERS: &[&str] = &[
    "codex", "gemini", "opencode", "claude", "droid", "agy", "kimi", "deepseek",
];

type IsPidAliveFn<'a> = &'a dyn Fn(u32) -> bool;
type ListTmuxSessionsFn<'a> = &'a dyn Fn() -> Vec<String>;
type FindAllZombieSessionsFn<'a> =
    &'a dyn Fn(IsPidAliveFn<'_>, Option<ListTmuxSessionsFn<'_>>) -> Vec<ZombieSession>;
type InputFn<'a> = &'a mut dyn FnMut(&str) -> io::Result<String>;
type KillTmuxSessionFn<'a> = &'a dyn Fn(&str) -> bool;

/// Information about a zombie session parsed from its tmux name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZombieSession {
    pub session: String,
    pub provider: String,
    pub parent_pid: u32,
}

/// Find CCB tmux sessions whose parent PID is no longer alive.
///
/// Mirrors Python `find_all_zombie_sessions(is_pid_alive, list_tmux_sessions_fn)`.
pub fn find_all_zombie_sessions(
    is_pid_alive: impl Fn(u32) -> bool,
    list_tmux_sessions_fn: Option<ListTmuxSessionsFn<'_>>,
) -> Vec<ZombieSession> {
    let list_fn = list_tmux_sessions_fn.unwrap_or(&list_tmux_sessions);
    let session_names = list_fn();
    if session_names.is_empty() {
        return Vec::new();
    }
    let mut zombies = Vec::new();
    for session in session_names {
        if let Some(zombie) = parse_zombie_session(&session, &is_pid_alive) {
            zombies.push(zombie);
        }
    }
    zombies
}

fn list_tmux_sessions() -> Vec<String> {
    if cfg!(windows) {
        return Vec::new();
    }
    let output = match Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
    {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_zombie_session(
    session: &str,
    is_pid_alive: &dyn Fn(u32) -> bool,
) -> Option<ZombieSession> {
    let (provider, after_provider) = session.split_once('-')?;
    if !ZOMBIE_PROVIDERS.contains(&provider) {
        return None;
    }
    let (parent_pid_text, _rest) = after_provider.split_once('-')?;
    let parent_pid = parent_pid_text.parse::<u32>().ok()?;
    if is_pid_alive(parent_pid) {
        return None;
    }
    Some(ZombieSession {
        session: session.to_string(),
        provider: provider.to_string(),
        parent_pid,
    })
}

/// Best-effort cleanup of zombie CCB tmux sessions.
///
/// Mirrors Python `kill_global_zombies(yes, is_pid_alive, find_all_zombie_sessions_fn, input_fn, kill_tmux_session_fn)`.
pub fn kill_global_zombies(
    yes: bool,
    is_pid_alive: impl Fn(u32) -> bool,
    find_all_zombie_sessions_fn: FindAllZombieSessionsFn<'_>,
    input_fn: Option<InputFn<'_>>,
    kill_tmux_session_fn: Option<KillTmuxSessionFn<'_>>,
) -> i32 {
    let kill_fn = kill_tmux_session_fn.unwrap_or(&kill_tmux_session);
    let zombies = find_all_zombie_sessions_fn(&is_pid_alive, None);
    if zombies.is_empty() {
        println!("✅ No zombie sessions found");
        return 0;
    }

    print_zombie_sessions(&zombies);

    if !confirm_cleanup(yes, input_fn) {
        return 1;
    }

    let (killed, failed) = cleanup_zombie_sessions(&zombies, kill_fn);
    print_cleanup_result(killed, failed);
    0
}

fn print_zombie_sessions(zombies: &[ZombieSession]) {
    println!("Found {} zombie session(s):", zombies.len());
    for zombie in zombies {
        println!(
            "  - {} (parent PID {} exited)",
            zombie.session, zombie.parent_pid
        );
    }
}

fn confirm_cleanup(yes: bool, input_fn: Option<InputFn<'_>>) -> bool {
    if yes {
        return true;
    }
    let mut fallback = String::new();
    let prompt = "\nClean up these sessions? [y/N] ";
    let result = match input_fn {
        Some(f) => f(prompt),
        None => {
            print!("{}", prompt);
            let _ = io::stdout().flush();
            io::stdin()
                .read_line(&mut fallback)
                .map(|_| fallback.clone())
        }
    };
    if let Ok(reply) = result {
        if reply.trim().eq_ignore_ascii_case("y") {
            return true;
        }
    }
    println!("❌ Cancelled");
    false
}

fn cleanup_zombie_sessions(
    zombies: &[ZombieSession],
    kill_tmux_session_fn: &dyn Fn(&str) -> bool,
) -> (usize, usize) {
    let mut killed = 0;
    let mut failed = 0;
    for zombie in zombies {
        if kill_tmux_session_fn(&zombie.session) {
            killed += 1;
        } else {
            failed += 1;
        }
    }
    (killed, failed)
}

fn kill_tmux_session(session_name: &str) -> bool {
    Command::new("tmux")
        .args(["kill-session", "-t", session_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn print_cleanup_result(killed: usize, failed: usize) {
    if failed > 0 {
        println!("✅ Cleaned up {killed} zombie session(s), {failed} failed");
    } else {
        println!("✅ Cleaned up {killed} zombie session(s)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zombie_session_detects_ccb_session_with_dead_parent() {
        let zombie = parse_zombie_session("codex-123-worker", &|_| false);
        assert_eq!(
            zombie,
            Some(ZombieSession {
                session: "codex-123-worker".to_string(),
                provider: "codex".to_string(),
                parent_pid: 123,
            })
        );
    }

    #[test]
    fn parse_zombie_session_ignores_alive_parent() {
        assert!(parse_zombie_session("codex-123-worker", &|_| true).is_none());
    }

    #[test]
    fn parse_zombie_session_ignores_non_ccb_session() {
        assert!(parse_zombie_session("demo-other", &|_| false).is_none());
    }

    #[test]
    fn find_all_zombie_sessions_filters_dead_parents() {
        let sessions: Vec<String> = vec![
            "codex-123-worker".to_string(),
            "claude-456-run".to_string(),
            "agy-789-debugger".to_string(),
            "demo-other".to_string(),
        ];
        let alive = |pid: u32| pid == 456;
        let zombies = find_all_zombie_sessions(alive, Some(&|| sessions.clone()));
        assert_eq!(zombies.len(), 2);
        assert!(zombies.iter().any(|z| z.session == "codex-123-worker"));
        assert!(zombies.iter().any(|z| z.session == "agy-789-debugger"));
    }

    #[test]
    fn kill_global_zombies_reports_partial_failures() {
        let find_fn = |_alive: &dyn Fn(u32) -> bool,
                       _list: Option<&dyn Fn() -> Vec<String>>|
         -> Vec<ZombieSession> {
            vec![
                ZombieSession {
                    session: "codex-123-worker".to_string(),
                    provider: "codex".to_string(),
                    parent_pid: 123,
                },
                ZombieSession {
                    session: "claude-234-run".to_string(),
                    provider: "claude".to_string(),
                    parent_pid: 234,
                },
            ]
        };
        let kill_fn = |name: &str| name == "codex-123-worker";
        let code = kill_global_zombies(true, |_| false, &find_fn, None, Some(&kill_fn));
        assert_eq!(code, 0);
    }
}
