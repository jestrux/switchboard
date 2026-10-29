//! The login shell commands run in. Its own file (its own process), because it clears
//! `SHELL`, which every other test's sessions read.

use sb_core::session::login_shell;

#[test]
fn without_shell_the_account_shell_is_used() {
    std::env::remove_var("SHELL");
    let shell = login_shell();
    let account = nix_account_shell();
    assert_eq!(shell, account, "a systemd service has no SHELL; the account's shell must win over the macOS default");
    assert!(std::path::Path::new(&shell).exists());

    std::env::set_var("SHELL", "/no/such/shell");
    assert_eq!(login_shell(), account, "a SHELL that doesn't exist is ignored");

    std::env::set_var("SHELL", "/bin/sh");
    assert_eq!(login_shell(), "/bin/sh");
}

/// The 7th field of this user's passwd entry, read without the library under test.
fn nix_account_shell() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let out = std::process::Command::new("getent").args(["passwd", &user]).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().rsplit(':').next().unwrap_or("").to_string(),
        // No getent (macOS): fall back to dscl's answer.
        _ => {
            let o = std::process::Command::new("dscl").args([".", "-read", &format!("/Users/{user}"), "UserShell"]).output().unwrap();
            String::from_utf8_lossy(&o.stdout).split_whitespace().last().unwrap_or("").to_string()
        }
    }
}
