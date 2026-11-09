//! Google installed-app OAuth: loopback redirect, PKCE, CSRF state, Secret Service storage.
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    process::Command,
};
use url::Url;

const SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

// Deliberately no Debug implementation: credentials must not appear in diagnostic logs.
#[derive(Clone, Deserialize, Serialize)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Deserialize, Serialize)]
struct Secret {
    refresh_token: String,
    client_id: String,
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
    refresh_token: Option<String>,
}

pub fn credentials(path: &str) -> Result<Credentials> {
    let path = path.trim();
    ensure!(
        !path.is_empty(),
        "Choose your Google OAuth Desktop app client JSON first. Use the Google Cloud setup link to create one."
    );
    let expanded = if let Some(suffix) = path.strip_prefix("~/") {
        dirs::home_dir()
            .context("Home directory not found")?
            .join(suffix)
    } else {
        Path::new(path).to_owned()
    };
    let data: serde_json::Value = serde_json::from_slice(
        &std::fs::read(expanded).context("Cannot read Google OAuth client JSON")?,
    )?;
    let installed = data.get("installed").context(
        "Choose a Google OAuth Desktop app client JSON (not a web client or service account)",
    )?;
    let credentials: Credentials = serde_json::from_value(installed.clone())
        .context("Client JSON is missing client_id or client_secret")?;
    ensure!(
        credentials
            .client_id
            .ends_with(".apps.googleusercontent.com")
            && !credentials.client_secret.is_empty(),
        "Invalid Google desktop client credentials"
    );
    Ok(credentials)
}

const PREVIOUS_APP_IDS: &[&str] = &[
    "io.github.cosmic_finance_dashboard.Applet",
    "io.github.dailywork.CosmicApplet",
];

async fn secret_tool(arguments: &[&str], input: Option<&str>) -> Result<Option<String>> {
    secret_for(crate::APP_ID, arguments, input).await
}

async fn secret_for(
    application: &str,
    arguments: &[&str],
    input: Option<&str>,
) -> Result<Option<String>> {
    let mut command = Command::new("secret-tool");
    command
        .args(arguments)
        .args(["application", application, "account", "google-calendar"])
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .context("Install libsecret (secret-tool) and unlock your desktop keyring")?;
    if let Some(input) = input {
        let mut stdin = child.stdin.take().context("Cannot open keyring input")?;
        stdin.write_all(input.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let output = tokio::time::timeout(Duration::from_secs(45), child.wait_with_output())
        .await
        .context("Keyring timed out; unlock your desktop keyring and try again")??;
    if !output.status.success() {
        if arguments.first() == Some(&"lookup") && output.status.code() == Some(1) {
            return Ok(None);
        }
        bail!("Cannot access the system keyring; unlock it and try again");
    }
    Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
}

async fn saved_secret() -> Result<Option<String>> {
    if let Some(value) = secret_tool(&["lookup"], None).await? {
        return Ok(Some(value));
    }
    for previous in PREVIOUS_APP_IDS {
        let Some(value) = secret_for(previous, &["lookup"], None).await? else {
            continue;
        };
        secret_tool(
            &[
                "store",
                "--label=Finance Dashboard \u{2014} Google Calendar",
            ],
            Some(&value),
        )
        .await?;
        secret_for(previous, &["clear"], None).await?;
        return Ok(Some(value));
    }
    Ok(None)
}

async fn store_secret(credentials: &Credentials, token: String) -> Result<()> {
    let value = serde_json::to_string(&Secret {
        refresh_token: token,
        client_id: credentials.client_id.clone(),
    })?;
    secret_tool(
        &["store", "--label=Finance Dashboard — Google Calendar"],
        Some(&value),
    )
    .await?;
    Ok(())
}

pub fn callback_code(target: &str, state: &str) -> Result<String> {
    ensure!(
        target.starts_with("/oauth/callback?"),
        "Unexpected OAuth callback path"
    );
    let url = Url::parse(&format!("http://127.0.0.1{target}"))?;
    let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    ensure!(
        params.get("state").is_some_and(|s| s == state),
        "OAuth state mismatch; sign-in was not accepted"
    );
    if params.contains_key("error") {
        bail!("Google sign-in was cancelled or access was denied");
    }
    params
        .get("code")
        .filter(|s| !s.is_empty())
        .cloned()
        .context("Google did not return an authorization code")
}

pub async fn sign_in(client: &Client, path: &str, email_hint: &str) -> Result<String> {
    let credentials = credentials(path)?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let redirect = format!(
        "http://127.0.0.1:{}/oauth/callback",
        listener.local_addr()?.port()
    );
    let verifier = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let state = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut authorization = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")?;
    authorization.query_pairs_mut().extend_pairs([
        ("client_id", credentials.client_id.as_str()),
        ("redirect_uri", &redirect),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    if !email_hint.trim().is_empty() {
        authorization
            .query_pairs_mut()
            .append_pair("login_hint", email_hint.trim());
    }
    open_url(authorization.as_str()).await?;
    let code = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let (stream, peer) = listener.accept().await?;
            if !peer.ip().is_loopback() { continue; }
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            tokio::time::timeout(Duration::from_secs(5), (&mut reader).take(8192).read_line(&mut request)).await??;
            let mut parts = request.split_whitespace();
            let method = parts.next().unwrap_or_default();
            let target = parts.next().unwrap_or_default();
            if method != "GET" || !target.starts_with("/oauth/callback?") {
                let _ = reader.get_mut().write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                continue;
            }
            let result = callback_code(target, &state);
            let text = if result.is_ok() { "Google authorization received. Return to Finance Dashboard to finish connecting." } else { "Sign-in was not accepted. Return to Finance Dashboard and try again." };
            let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{text}", text.len());
            let _ = reader.get_mut().write_all(reply.as_bytes()).await;
            return result;
        }
    }).await.context("Google sign-in timed out after three minutes; try again")??;

    let token: Token = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?
        .error_for_status()
        .context("Google token exchange failed; check the OAuth client and Calendar API access")?
        .json()
        .await?;
    let primary: serde_json::Value = client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary")
        .bearer_auth(&token.access_token)
        .send()
        .await?
        .error_for_status()
        .context("Enable Google Calendar API for this OAuth project")?
        .json()
        .await?;
    let email = primary["id"]
        .as_str()
        .context("Google did not identify the primary calendar")?
        .to_owned();
    let refresh = token.refresh_token.context("Google did not issue a refresh token; remove this app from Google account connections and reconnect")?;
    crate::config::private_write(
        &crate::config::config_dir().join("google-client.json"),
        &serde_json::to_vec(&serde_json::json!({"installed": credentials}))?,
    )?;
    store_secret(&credentials, refresh).await?;
    Ok(email)
}

pub async fn access_token(client: &Client, path: &str) -> Result<String> {
    let credentials = credentials(path)?;
    let value = saved_secret()
        .await?
        .context("Google Calendar is not connected; sign in from Settings")?;
    let secret: Secret = serde_json::from_str(&value)
        .context("Saved Google connection is invalid; reconnect in Settings")?;
    ensure!(
        credentials.client_id == secret.client_id,
        "OAuth client changed; reconnect Google Calendar in Settings"
    );
    let token: Token = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("refresh_token", secret.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?
        .error_for_status()
        .context("Google authorization expired or was revoked; reconnect in Settings")?
        .json()
        .await?;
    if let Some(refresh) = token.refresh_token {
        store_secret(&credentials, refresh).await?;
    }
    Ok(token.access_token)
}

pub async fn disconnect() -> Result<()> {
    for application in std::iter::once(crate::APP_ID).chain(PREVIOUS_APP_IDS.iter().copied()) {
        if secret_for(application, &["lookup"], None).await?.is_some() {
            secret_for(application, &["clear"], None).await?;
        }
    }
    Ok(())
}

pub async fn open_url(value: &str) -> Result<()> {
    let url = Url::parse(value)?;
    ensure!(
        url.scheme() == "https",
        "Only HTTPS browser links are supported"
    );
    // The panel embeds applets using a private inherited socket. External apps
    // must connect to the desktop compositor, not inherit that panel connection.
    let mut command = Command::new("xdg-open");
    command.env_remove("WAYLAND_SOCKET");
    if let Ok(display) = std::env::var("HOST_WAYLAND_DISPLAY") {
        command.env("WAYLAND_DISPLAY", display);
    }
    let status = command
        .arg(url.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("Cannot open the default web browser")?;
    ensure!(
        status.success(),
        "The default web browser could not open the link"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_client_reports_setup_step_before_browser_launch() {
        let error = credentials("   ").err().expect("Missing client must fail");
        assert!(
            error
                .to_string()
                .contains("Choose your Google OAuth Desktop app client JSON first")
        );
    }
    #[test]
    fn oauth_requires_matching_state_and_callback_path() {
        assert_eq!(
            callback_code("/oauth/callback?state=secret&code=a%2Fb", "secret").unwrap(),
            "a/b"
        );
        for target in [
            "/oauth/callback?state=wrong&code=abc",
            "/oauth/callback?code=abc",
            "/other?state=secret&code=abc",
            "/oauth/callback?state=secret&error=access_denied",
        ] {
            assert!(callback_code(target, "secret").is_err());
        }
    }
}
