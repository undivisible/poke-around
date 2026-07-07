use crate::{Error, Result, bridge_state};
use rs_poke::{CredentialsStore, LoginOptions, Poke, PokeOptions};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OAuthRecoveryOutcome {
    Restart { token: String },
    Failed(String),
}

pub(crate) async fn ensure_auth(force_fresh: bool) -> Result<String> {
    if !force_fresh {
        if let Some(token) = rs_poke::get_token()? {
            return Ok(token);
        }
    } else {
        rs_poke::logout()
            .await
            .map_err(|err| Error::msg(err.to_string()))?;
    }
    bridge_state::log_status("Opening browser for Poke login...");
    let store = CredentialsStore::default_store().map_err(|err| Error::msg(err.to_string()))?;
    let options = LoginOptions::new(store).on_code(|info| {
        bridge_state::log_status(&format!(
            "Enter code {} at {}",
            info.user_code, info.login_url
        ));
    });
    let login = if force_fresh {
        rs_poke::login_fresh(options).await
    } else {
        rs_poke::login(options).await
    };
    login
        .map(|result| result.token)
        .map_err(|err| Error::msg(err.to_string()))
}

pub(crate) fn plan_oauth_recovery(
    cached_reauth: std::result::Result<String, String>,
    fresh_login: std::result::Result<String, String>,
) -> OAuthRecoveryOutcome {
    match cached_reauth {
        Ok(token) => OAuthRecoveryOutcome::Restart { token },
        Err(cached_err) => match fresh_login {
            Ok(token) => OAuthRecoveryOutcome::Restart { token },
            Err(fresh_err) => OAuthRecoveryOutcome::Failed(format!(
                "{cached_err}; fresh login failed: {fresh_err}"
            )),
        },
    }
}

pub(crate) async fn recover_from_oauth_required() -> OAuthRecoveryOutcome {
    let cached = ensure_auth(false).await.map_err(|err| err.to_string());
    if cached.is_ok() {
        return plan_oauth_recovery(cached, Err("skipped".into()));
    }
    bridge_state::log_status("Cached credentials invalid - opening browser for fresh Poke login...");
    let fresh = ensure_auth(true).await.map_err(|err| err.to_string());
    plan_oauth_recovery(cached, fresh)
}

pub(crate) fn make_poke(token: &str) -> Result<Poke> {
    Poke::new(PokeOptions {
        api_key: Some(token.to_string()),
        ..PokeOptions::default()
    })
    .map_err(|err| Error::msg(err.to_string()))
}

#[cfg(target_os = "windows")]
pub(crate) fn oauth_failure_hint() -> &'static str {
    " On Windows, run poke-around from an interactive desktop terminal (not session 0 or a service) so browser login can open."
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn oauth_failure_hint() -> &'static str {
    ""
}

pub(crate) fn ensure_integration_name(base: &str) -> Result<String> {
    let state = bridge_state::read_state()?;
    if let Some(name) = state.get("integrationName").and_then(Value::as_str)
        && !name.is_empty()
    {
        return Ok(name.to_string());
    }
    let name = compute_integration_name(base);
    bridge_state::patch_state([("integrationName", Value::String(name.clone()))])?;
    Ok(name)
}

fn compute_integration_name(base: &str) -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .unwrap_or_default()
        });
    let suffix = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if suffix.is_empty() {
        base.to_string()
    } else {
        format!("{base}-{suffix}")
    }
}
