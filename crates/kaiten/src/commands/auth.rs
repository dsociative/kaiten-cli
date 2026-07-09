use std::io::Write;

use kaiten_client::KaitenClient;

use crate::cli::AuthCmd;
use crate::config::{self, FileConfig, TokenSource};
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: AuthCmd, json: bool) -> Result<(), CliError> {
    match cmd {
        AuthCmd::Login { domain, token } => login(domain, token).await,
        AuthCmd::Status => status(json).await,
    }
}

async fn login(domain: Option<String>, token: Option<String>) -> Result<(), CliError> {
    let domain = match domain {
        Some(domain) => domain,
        None => prompt_line("Kaiten domain (as in https://<domain>.kaiten.ru): ")?,
    };
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err(CliError::InvalidArg("domain must not be empty".into()));
    }
    let token = match token {
        Some(token) => token,
        None => rpassword::prompt_password("API token: ")?,
    };
    if token.is_empty() {
        return Err(CliError::InvalidArg("token must not be empty".into()));
    }
    // KAITEN_BASE_URL is honored so login can be pointed at a mock server in tests.
    let base_url = std::env::var("KAITEN_BASE_URL")
        .unwrap_or_else(|_| format!("https://{domain}.kaiten.ru/api/latest"));
    let client = KaitenClient::new(&base_url, &token)?;
    let user = client.users().current().await?;

    let mut file = FileConfig::load()?;
    file.domain = Some(domain.clone());
    file.token = Some(token);
    file.save()?;

    println!("Logged in to {domain}.kaiten.ru as {}", user_label(&user));
    Ok(())
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
            "token_source": source,
            "user": user,
        }));
    }
    println!("domain:       {}", domain.as_deref().unwrap_or("-"));
    println!("base_url:     {}", resolved.base_url);
    println!("token source: {source}");
    println!("logged in as: {} (id {})", user_label(&user), user.id);
    Ok(())
}

fn user_label(user: &kaiten_client::User) -> String {
    user.username
        .clone()
        .or_else(|| user.full_name.clone())
        .unwrap_or_else(|| user.id.to_string())
}

fn prompt_line(prompt: &str) -> Result<String, CliError> {
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
