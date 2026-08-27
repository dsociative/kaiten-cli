use std::io::Write;

use kaiten_client::{KaitenClient, KaitenError};

use crate::cli::AuthCmd;
use crate::config::{self, FileConfig, TokenSource};
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: AuthCmd, json: bool) -> Result<(), CliError> {
    match cmd {
        AuthCmd::Login {
            domain,
            base_url,
            token,
        } => login(domain, base_url, token).await,
        AuthCmd::Status => status(json).await,
    }
}

/// What `auth login` authenticates against: a Kaiten cloud company by domain,
/// or an on-premise installation by its full API base URL (issue #13).
enum LoginTarget {
    Domain(String),
    BaseUrl(String),
}

async fn login(
    domain: Option<String>,
    base_url: Option<String>,
    token: Option<String>,
) -> Result<(), CliError> {
    let target = match (domain, base_url) {
        (_, Some(url)) => LoginTarget::BaseUrl(validate_base_url(&url)?),
        (domain, None) => {
            let domain = match domain {
                Some(domain) => domain,
                None => prompt_line("Kaiten domain (as in https://<domain>.kaiten.ru): ")?,
            };
            let domain = domain.trim().to_string();
            if domain.is_empty() {
                return Err(CliError::InvalidArg("domain must not be empty".into()));
            }
            LoginTarget::Domain(domain)
        }
    };
    let token = match token {
        Some(token) => token,
        None => rpassword::prompt_password("API token: ")?,
    };
    if token.is_empty() {
        return Err(CliError::InvalidArg("token must not be empty".into()));
    }
    // Precedence: --base-url, then KAITEN_BASE_URL (so login can be pointed at
    // a mock server in tests), then the cloud URL derived from the domain.
    let api_base = match &target {
        LoginTarget::BaseUrl(url) => url.clone(),
        LoginTarget::Domain(domain) => {
            std::env::var("KAITEN_BASE_URL").unwrap_or_else(|_| config::domain_base_url(domain))
        }
    };
    let client = KaitenClient::new(&api_base, &token)?;
    let user = match client.users().current().await {
        Ok(user) => user,
        Err(err) => return Err(login_error(&target, err)),
    };

    let mut file = FileConfig::load()?;
    // Keep exactly one of domain/base_url so `auth status` never shows a stale pair.
    match &target {
        LoginTarget::BaseUrl(url) => {
            file.base_url = Some(url.clone());
            file.domain = None;
        }
        LoginTarget::Domain(domain) => {
            file.domain = Some(domain.clone());
            file.base_url = None;
        }
    }
    file.token = Some(token);
    file.save()?;

    match &target {
        LoginTarget::BaseUrl(url) => {
            println!("Logged in to {url} as {}", output::user_label(&user));
        }
        LoginTarget::Domain(domain) => {
            println!(
                "Logged in to {domain}.kaiten.ru as {}",
                output::user_label(&user)
            );
        }
    }
    Ok(())
}

/// `--base-url` must be an absolute URL; the trailing slash is dropped so the
/// client can append `/users/current` and friends.
fn validate_base_url(raw: &str) -> Result<String, CliError> {
    let url = raw.trim().trim_end_matches('/');
    match url::Url::parse(url) {
        Ok(parsed) if parsed.has_host() && ["http", "https"].contains(&parsed.scheme()) => {
            Ok(url.to_string())
        }
        Ok(_) => Err(CliError::InvalidArg(format!(
            "--base-url is not a URL: {raw} (expected https://host/api/latest)"
        ))),
        Err(e) => Err(CliError::InvalidArg(format!(
            "--base-url is not a URL: {raw}: {e} (expected https://host/api/latest)"
        ))),
    }
}

/// A 404 from `/users/current` on an explicit base URL almost always means
/// the API prefix is missing from it.
fn login_error(target: &LoginTarget, err: KaitenError) -> CliError {
    match (target, &err) {
        (LoginTarget::BaseUrl(_), KaitenError::Api { status: 404, .. }) => CliError::Config(
            format!("{err}; base URL must include the API prefix, e.g. https://host/api/latest"),
        ),
        _ => CliError::Api(err),
    }
}

async fn status(json: bool) -> Result<(), CliError> {
    let resolved = config::resolve()?;
    let file = FileConfig::load()?;
    let domain = std::env::var("KAITEN_DOMAIN").ok().or(file.domain);
    let client = KaitenClient::new(&resolved.base_url, &resolved.token)?;
    let user = client.users().current().await?;
    let source = match resolved.token_source {
        TokenSource::Env => "env",
        TokenSource::File => "file",
    };
    if json {
        return output::print_json(&serde_json::json!({
            "domain": domain,
            "base_url": resolved.base_url,
            "base_url_source": resolved.base_url_source.as_str(),
            "token_source": source,
            "user": user,
        }));
    }
    println!("domain:       {}", domain.as_deref().unwrap_or("-"));
    println!("base_url:     {}", resolved.base_url);
    println!("url source:   {}", resolved.base_url_source.as_str());
    println!("token source: {source}");
    println!(
        "logged in as: {} (id {})",
        output::user_label(&user),
        user.id
    );
    Ok(())
}

fn prompt_line(prompt: &str) -> Result<String, CliError> {
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
