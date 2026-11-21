// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Active screen time pauses while systemd-logind reports the session idle or locked.

pub fn session_is_active(text: &str) -> bool {
    let mut idle = false;
    let mut locked = false;
    let mut state_active = true;
    for line in text.lines() {
        if let Some(value) = line.trim().strip_prefix("IdleHint=") {
            idle = value.eq_ignore_ascii_case("yes");
        } else if let Some(value) = line.trim().strip_prefix("LockedHint=") {
            locked = value.eq_ignore_ascii_case("yes");
        } else if let Some(value) = line.trim().strip_prefix("State=") {
            state_active = value == "active";
        }
    }
    !idle && !locked && state_active
}

pub async fn probe() -> Result<bool, String> {
    let output = tokio::process::Command::new("loginctl")
        .args([
            "show-session",
            "auto",
            "-p",
            "IdleHint",
            "-p",
            "LockedHint",
            "-p",
            "State",
        ])
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "loginctl exited with {}",
            output.status.code().unwrap_or(-1)
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(session_is_active(&text))
}

#[cfg(test)]
mod tests {
    use super::session_is_active;

    #[test]
    fn idle_and_lock_pause_screen_time() {
        assert!(session_is_active(
            "State=active\nIdleHint=no\nLockedHint=no\n"
        ));
        assert!(!session_is_active(
            "State=active\nIdleHint=yes\nLockedHint=no\n"
        ));
        assert!(!session_is_active(
            "State=active\nIdleHint=no\nLockedHint=yes\n"
        ));
        assert!(!session_is_active(
            "State=closing\nIdleHint=no\nLockedHint=no\n"
        ));
    }
}
