use ccbr_types::env::{env_bool, env_float, env_int};
use serial_test::serial;

#[test]
#[serial]
fn test_env_bool_truthy_and_falsy() {
    std::env::remove_var("X");
    assert!(env_bool("X", true));
    assert!(!env_bool("X", false));

    for v in ["1", "true", "yes", "on", " TRUE ", "Yes"] {
        std::env::set_var("X", v);
        assert!(env_bool("X", false));
    }

    for v in ["0", "false", "no", "off", " 0 ", "False"] {
        std::env::set_var("X", v);
        assert!(!env_bool("X", true));
    }

    std::env::set_var("X", "maybe");
    assert!(env_bool("X", true));
    assert!(!env_bool("X", false));
}

#[test]
#[serial]
fn test_env_bool_empty_string_uses_default() {
    std::env::set_var("X", "");
    assert!(env_bool("X", true));
    assert!(!env_bool("X", false));
}

#[test]
#[serial]
fn test_env_int_parsing() {
    std::env::remove_var("X");
    assert_eq!(env_int("X", 7), 7);

    std::env::set_var("X", " 42 ");
    assert_eq!(env_int("X", 7), 42);

    std::env::set_var("X", "bad");
    assert_eq!(env_int("X", 7), 7);
}

#[test]
#[serial]
fn test_env_float_parsing() {
    std::env::remove_var("X");
    assert!((env_float("X", 1.5) - 1.5).abs() < f64::EPSILON);

    std::env::set_var("X", " 2.75 ");
    assert!((env_float("X", 1.5) - 2.75).abs() < f64::EPSILON);

    std::env::set_var("X", "bad");
    assert!((env_float("X", 1.5) - 1.5).abs() < f64::EPSILON);
}
