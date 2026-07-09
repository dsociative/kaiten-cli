use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FileConfig {
    pub domain: Option<String>,
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

#[derive(Debug)]
pub struct Resolved {
    pub base_url: String,
    pub token: String,
    // Read starting with `auth status` (Task 11); unused until then.
    #[allow(dead_code)]
    pub token_source: TokenSource,
    pub defaults: Defaults,
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
        dirs::home_dir()
            .map(|home| home.join(".config").join("kaiten"))
            .unwrap_or_else(|| PathBuf::from(".config/kaiten"))
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
}

pub fn resolve() -> Result<Resolved, CliError> {
    let env: HashMap<String, String> = std::env::vars().collect();
    resolve_from(FileConfig::load()?, &env)
}

/// Приоритет: env (KAITEN_TOKEN/KAITEN_DOMAIN/KAITEN_BASE_URL) → файл.
pub fn resolve_from(file: FileConfig, env: &HashMap<String, String>) -> Result<Resolved, CliError> {
    let (token, token_source) = match env.get("KAITEN_TOKEN").filter(|t| !t.is_empty()) {
        Some(token) => (token.clone(), TokenSource::Env),
        None => match file.token {
            Some(token) => (token, TokenSource::File),
            None => {
                return Err(CliError::Config(
                    "no token: run `kaiten auth login` or set KAITEN_TOKEN".into(),
                ));
            }
        },
    };
    let base_url = match env.get("KAITEN_BASE_URL").filter(|u| !u.is_empty()) {
        Some(url) => url.trim_end_matches('/').to_string(),
        None => {
            let domain = env
                .get("KAITEN_DOMAIN")
                .filter(|d| !d.is_empty())
                .cloned()
                .or(file.domain);
            match domain {
                Some(domain) => format!("https://{domain}.kaiten.ru/api/latest"),
                None => {
                    return Err(CliError::Config(
                        "no domain: run `kaiten auth login` or set KAITEN_DOMAIN".into(),
                    ));
                }
            }
        }
    };
    Ok(Resolved {
        base_url,
        token,
        token_source,
        defaults: file.defaults,
    })
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
}
