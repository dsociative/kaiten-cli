use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FileConfig {
    /// Kaiten cloud company: `https://{domain}.kaiten.ru/api/latest`.
    pub domain: Option<String>,
    /// Full API base URL of an on-premise installation, e.g.
    /// `https://kaiten.corp.local/api/latest`; wins over `domain`. Declared
    /// before `defaults`: toml writes fields in order and cannot emit a scalar
    /// after the `[defaults]` table.
    pub base_url: Option<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Defaults {
    pub space: Option<u64>,
    pub board: Option<u64>,
}

#[derive(Debug)]
pub enum TokenSource {
    Env,
    File,
}

/// Where the API base URL came from — shown by `auth status` (table and
/// `--json`), so the values are part of the CLI contract.
#[derive(Debug, Clone, Copy)]
pub enum BaseUrlSource {
    /// `KAITEN_BASE_URL`
    Env,
    /// `base_url` in config.toml
    File,
    /// Derived from `KAITEN_DOMAIN`
    EnvDomain,
    /// Derived from `domain` in config.toml
    FileDomain,
}

impl BaseUrlSource {
    pub fn as_str(self) -> &'static str {
        match self {
            BaseUrlSource::Env => "env",
            BaseUrlSource::File => "file",
            BaseUrlSource::EnvDomain => "env-domain",
            BaseUrlSource::FileDomain => "file-domain",
        }
    }
}

#[derive(Debug)]
pub struct Resolved {
    pub base_url: String,
    pub base_url_source: BaseUrlSource,
    pub token: String,
    pub token_source: TokenSource,
    pub defaults: Defaults,
}

/// API base of a Kaiten cloud company.
pub fn domain_base_url(domain: &str) -> String {
    format!("https://{domain}.kaiten.ru/api/latest")
}

impl FileConfig {
    /// $KAITEN_CONFIG_DIR || $XDG_CONFIG_HOME/kaiten || ~/.config/kaiten
    pub fn dir() -> PathBuf {
        if let Ok(dir) = std::env::var("KAITEN_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("kaiten");
        }
        dirs::home_dir().map_or_else(
            || PathBuf::from(".config/kaiten"),
            |home| home.join(".config").join("kaiten"),
        )
    }

    /// Отсутствие файла — не ошибка: возвращается Default.
    pub fn load() -> Result<FileConfig, CliError> {
        let path = Self::dir().join("config.toml");
        let body = match std::fs::read_to_string(&path) {
            Ok(body) => body,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(FileConfig::default());
            }
            Err(err) => return Err(CliError::Io(err)),
        };
        toml::from_str(&body)
            .map_err(|err| CliError::Config(format!("invalid config {}: {err}", path.display())))
    }

    /// Создаёт каталог, пишет config.toml с правами 0600 (unix) без окна с
    /// более широкими правами: файл создаётся сразу с mode 0o600, а если он
    /// уже существовал с более слабыми правами, они ужесточаются до записи
    /// содержимого.
    pub fn save(&self) -> Result<(), CliError> {
        let dir = Self::dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let body = toml::to_string_pretty(self)
            .map_err(|err| CliError::Config(format!("failed to serialize config: {err}")))?;
        #[cfg(unix)]
        {
            use std::io::Write as _;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            // `mode()` only applies when the file is created. If config.toml
            // already existed with looser permissions, tighten them here,
            // before any content (the token) is written.
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            file.write_all(body.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, body)?;
        }
        Ok(())
    }
}

pub fn resolve() -> Result<Resolved, CliError> {
    let env: HashMap<String, String> = std::env::vars().collect();
    resolve_from(FileConfig::load()?, &env)
}

/// Приоритет: env (KAITEN_TOKEN/KAITEN_DOMAIN/KAITEN_BASE_URL) → файл.
/// Для base URL: KAITEN_BASE_URL → KAITEN_DOMAIN → base_url из файла →
/// domain из файла (env всегда важнее файла).
pub fn resolve_from(file: FileConfig, env: &HashMap<String, String>) -> Result<Resolved, CliError> {
    let FileConfig {
        domain,
        base_url,
        token,
        defaults,
    } = file;
    let (token, token_source) = match env.get("KAITEN_TOKEN").filter(|t| !t.is_empty()) {
        Some(token) => (token.clone(), TokenSource::Env),
        None => match token {
            Some(token) => (token, TokenSource::File),
            None => {
                return Err(CliError::Config(
                    "no token: run `kaiten auth login` or set KAITEN_TOKEN".into(),
                ));
            }
        },
    };
    let (base_url, base_url_source) =
        resolve_base_url(domain.as_deref(), base_url.as_deref(), env)?;
    Ok(Resolved {
        base_url,
        base_url_source,
        token,
        token_source,
        defaults,
    })
}

/// `KAITEN_BASE_URL` → `KAITEN_DOMAIN` → `base_url` from the file → `domain`
/// from the file: the environment always beats the file.
fn resolve_base_url(
    file_domain: Option<&str>,
    file_base_url: Option<&str>,
    env: &HashMap<String, String>,
) -> Result<(String, BaseUrlSource), CliError> {
    if let Some(url) = env.get("KAITEN_BASE_URL").filter(|u| !u.is_empty()) {
        return Ok((url.trim_end_matches('/').to_string(), BaseUrlSource::Env));
    }
    if let Some(domain) = env.get("KAITEN_DOMAIN").filter(|d| !d.is_empty()) {
        return Ok((domain_base_url(domain), BaseUrlSource::EnvDomain));
    }
    if let Some(url) = file_base_url.filter(|u| !u.is_empty()) {
        return Ok((url.trim_end_matches('/').to_string(), BaseUrlSource::File));
    }
    if let Some(domain) = file_domain {
        return Ok((domain_base_url(domain), BaseUrlSource::FileDomain));
    }
    Err(CliError::Config(
        "no domain or base URL: run `kaiten auth login` or set KAITEN_DOMAIN / KAITEN_BASE_URL"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn env_overrides_file() {
        let file = FileConfig {
            domain: Some("filedomain".into()),
            base_url: None,
            token: Some("file-token".into()),
            defaults: Defaults::default(),
        };
        let env = env(&[
            ("KAITEN_TOKEN", "env-token"),
            ("KAITEN_DOMAIN", "envdomain"),
        ]);
        let resolved = resolve_from(file, &env).unwrap();
        assert_eq!(resolved.token, "env-token");
        assert!(matches!(resolved.token_source, TokenSource::Env));
        assert_eq!(resolved.base_url, "https://envdomain.kaiten.ru/api/latest");
    }

    #[test]
    fn file_values_used_when_env_empty() {
        let file = FileConfig {
            domain: Some("mycompany".into()),
            base_url: None,
            token: Some("file-token".into()),
            defaults: Defaults::default(),
        };
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.token, "file-token");
        assert!(matches!(resolved.token_source, TokenSource::File));
        assert_eq!(resolved.base_url, "https://mycompany.kaiten.ru/api/latest");
    }

    #[test]
    fn base_url_env_wins_over_domain_and_needs_no_domain() {
        let env = env(&[
            ("KAITEN_TOKEN", "t"),
            ("KAITEN_BASE_URL", "http://127.0.0.1:9999/"),
        ]);
        let resolved = resolve_from(FileConfig::default(), &env).unwrap();
        assert_eq!(resolved.base_url, "http://127.0.0.1:9999");
    }

    #[test]
    fn missing_token_is_clear_error() {
        let env = env(&[("KAITEN_DOMAIN", "mycompany")]);
        let err = resolve_from(FileConfig::default(), &env).unwrap_err();
        assert!(matches!(err, CliError::Config(_)));
        assert!(err.to_string().contains("no token"), "{err}");
    }

    #[test]
    fn missing_domain_is_clear_error() {
        let env = env(&[("KAITEN_TOKEN", "t")]);
        let err = resolve_from(FileConfig::default(), &env).unwrap_err();
        assert!(err.to_string().contains("no domain"), "{err}");
    }

    #[test]
    fn defaults_parse_from_toml() {
        let file: FileConfig = toml::from_str(
            "domain = \"mycompany\"\ntoken = \"t\"\n\n[defaults]\nspace = 123\nboard = 456\n",
        )
        .unwrap();
        assert_eq!(file.defaults.space, Some(123));
        assert_eq!(file.defaults.board, Some(456));
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.defaults.board, Some(456));
    }
    // --- issue #13: on-premise via a config-file `base_url` ---

    #[test]
    fn file_base_url_used_when_env_empty_and_trailing_slash_trimmed() {
        let file = FileConfig {
            domain: None,
            base_url: Some("https://kaiten.corp.local/api/latest/".into()),
            token: Some("t".into()),
            defaults: Defaults::default(),
        };
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.base_url, "https://kaiten.corp.local/api/latest");
        assert!(matches!(resolved.base_url_source, BaseUrlSource::File));
    }

    #[test]
    fn file_base_url_beats_file_domain() {
        let file = FileConfig {
            domain: Some("mycompany".into()),
            base_url: Some("https://kaiten.corp.local/api/latest".into()),
            token: Some("t".into()),
            defaults: Defaults::default(),
        };
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.base_url, "https://kaiten.corp.local/api/latest");
    }

    #[test]
    fn env_domain_beats_file_base_url() {
        let file = FileConfig {
            domain: None,
            base_url: Some("https://kaiten.corp.local/api/latest".into()),
            token: Some("t".into()),
            defaults: Defaults::default(),
        };
        let env = env(&[("KAITEN_DOMAIN", "envdomain")]);
        let resolved = resolve_from(file, &env).unwrap();
        assert_eq!(resolved.base_url, "https://envdomain.kaiten.ru/api/latest");
        assert!(matches!(resolved.base_url_source, BaseUrlSource::EnvDomain));
        assert_eq!(resolved.base_url_source.as_str(), "env-domain");
    }

    #[test]
    fn env_base_url_beats_everything() {
        let file = FileConfig {
            domain: Some("mycompany".into()),
            base_url: Some("https://kaiten.corp.local/api/latest".into()),
            token: Some("t".into()),
            defaults: Defaults::default(),
        };
        let env = env(&[
            ("KAITEN_BASE_URL", "http://127.0.0.1:9999"),
            ("KAITEN_DOMAIN", "envdomain"),
        ]);
        let resolved = resolve_from(file, &env).unwrap();
        assert_eq!(resolved.base_url, "http://127.0.0.1:9999");
        assert!(matches!(resolved.base_url_source, BaseUrlSource::Env));
    }

    #[test]
    fn domain_source_is_reported_for_file_domain() {
        let file = FileConfig {
            domain: Some("mycompany".into()),
            base_url: None,
            token: Some("t".into()),
            defaults: Defaults::default(),
        };
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert!(matches!(
            resolved.base_url_source,
            BaseUrlSource::FileDomain
        ));
        assert_eq!(resolved.base_url_source.as_str(), "file-domain");
    }

    /// `toml` serializes fields in declaration order and cannot emit a scalar
    /// after the `[defaults]` table — pins that `base_url` sits before it.
    #[test]
    fn base_url_round_trips_through_toml_with_defaults() {
        let file = FileConfig {
            domain: None,
            base_url: Some("https://kaiten.corp.local/api/latest".into()),
            token: Some("t".into()),
            defaults: Defaults {
                space: Some(1),
                board: None,
            },
        };
        let body = toml::to_string_pretty(&file).unwrap();
        let back: FileConfig = toml::from_str(&body).unwrap();
        assert_eq!(
            back.base_url.as_deref(),
            Some("https://kaiten.corp.local/api/latest")
        );
        assert_eq!(back.domain, None);
        assert_eq!(back.defaults.space, Some(1));
        assert!(!body.contains("domain"), "{body}");
    }

    #[test]
    fn missing_domain_error_mentions_base_url_too() {
        let env = env(&[("KAITEN_TOKEN", "t")]);
        let err = resolve_from(FileConfig::default(), &env).unwrap_err();
        assert!(err.to_string().contains("no domain"), "{err}");
        assert!(err.to_string().contains("KAITEN_BASE_URL"), "{err}");
    }
}
