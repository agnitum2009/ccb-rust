use ccbr_types::ui::i18n::{self, Language};
use serial_test::serial;

fn reset_language() {
    i18n::set_lang(Language::English);
}

#[test]
#[serial]
fn test_detect_language_from_ccbr_lang() {
    std::env::remove_var("CCBR_LANG");
    std::env::remove_var("LANG");
    std::env::remove_var("LC_ALL");
    std::env::remove_var("LC_MESSAGES");

    std::env::set_var("CCBR_LANG", "zh");
    assert_eq!(i18n::detect_language(), Language::Chinese);

    std::env::set_var("CCBR_LANG", "CN");
    assert_eq!(i18n::detect_language(), Language::Chinese);

    std::env::set_var("CCBR_LANG", "en");
    assert_eq!(i18n::detect_language(), Language::English);

    std::env::set_var("CCBR_LANG", "auto");
    assert_eq!(i18n::detect_language(), Language::English);
}

#[test]
#[serial]
fn test_detect_language_from_system_locale() {
    std::env::remove_var("CCBR_LANG");

    std::env::set_var("LANG", "zh_CN.UTF-8");
    assert_eq!(i18n::detect_language(), Language::Chinese);

    std::env::set_var("LANG", "en_US.UTF-8");
    assert_eq!(i18n::detect_language(), Language::English);
}

#[test]
#[serial]
fn test_get_and_set_lang() {
    reset_language();
    i18n::set_lang(Language::Chinese);
    assert_eq!(i18n::get_lang(), Language::Chinese);

    i18n::set_lang(Language::English);
    assert_eq!(i18n::get_lang(), Language::English);
}

#[test]
#[serial]
fn test_t_english_translations() {
    reset_language();
    i18n::set_lang(Language::English);

    assert_eq!(
        i18n::t("no_terminal_backend", &[]),
        "No tmux backend detected"
    );
    assert_eq!(
        i18n::t(
            "starting_backend",
            &[("provider", "codex"), ("terminal", "tmux")]
        ),
        "Starting codex backend (tmux)..."
    );
}

#[test]
#[serial]
fn test_t_chinese_translations() {
    reset_language();
    i18n::set_lang(Language::Chinese);

    assert_eq!(i18n::t("no_terminal_backend", &[]), "未检测到 tmux 后端");
    assert_eq!(
        i18n::t(
            "starting_backend",
            &[("provider", "codex"), ("terminal", "tmux")]
        ),
        "正在启动 codex 后端 (tmux)..."
    );
}

#[test]
#[serial]
fn test_t_falls_back_to_key_for_unknown() {
    reset_language();
    i18n::set_lang(Language::English);

    assert_eq!(i18n::t("totally_unknown_key", &[]), "totally_unknown_key");
}

#[test]
#[serial]
fn test_t_missing_placeholder_is_left_unexpanded() {
    reset_language();
    i18n::set_lang(Language::English);

    assert_eq!(
        i18n::t("starting_backend", &[("provider", "codex")]),
        "Starting codex backend ({terminal})..."
    );
}
