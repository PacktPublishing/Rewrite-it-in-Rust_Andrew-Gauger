//! Settings, from the environment or from an env file named by
//! `CAIRN_AUTH_ENV` (the errata's `chapter9/cairn-auth/.env`, written by
//! `setup-entra.sh`). The environment wins over the file.

use std::collections::HashMap;
use std::path::Path;

pub struct Config {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub database_url: String,
    pub listen: String,
    pub session_secs: f64,
}

impl Config {
    pub fn load() -> Result<Config, String> {
        let file = match std::env::var("CAIRN_AUTH_ENV") {
            Ok(path) => read_env_file(Path::new(&path))?,
            Err(_) => HashMap::new(),
        };
        let get = |key: &str| {
            std::env::var(key).ok().or_else(|| file.get(key).cloned())
        };
        let need = |key: &str| {
            get(key).ok_or_else(|| format!("{key} is not set"))
        };
        Ok(Config {
            issuer: need("CAIRN_AUTH_ISSUER")?,
            client_id: need("CAIRN_AUTH_CLIENT_ID")?,
            client_secret: need("CAIRN_AUTH_CLIENT_SECRET")?,
            redirect_uri: need("CAIRN_AUTH_REDIRECT_URI")?,
            database_url: get("CAIRN_AUTH_DATABASE_URL").unwrap_or_else(|| {
                "postgres://cairn_auth:cairn_auth_dev@127.0.0.1:5433/cairn"
                    .into()
            }),
            listen: get("CAIRN_AUTH_LISTEN")
                .unwrap_or_else(|| "127.0.0.1:3000".into()),
            session_secs: match get("CAIRN_SESSION_SECONDS") {
                Some(s) => s.parse().map_err(|_| {
                    format!("CAIRN_SESSION_SECONDS isn't a number: {s}")
                })?,
                None => 4.0 * 60.0 * 60.0,
            },
        })
    }
}

/// `KEY=value` lines; blank lines and `#` comments skipped.
fn read_env_file(
    path: &Path,
) -> Result<HashMap<String, String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect())
}
