use kaiten_client::KaitenClient;

use crate::error::CliError;

pub async fn run(
    client: &KaitenClient,
    method: &str,
    path: &str,
    data: Option<String>,
) -> Result<(), CliError> {
    let method = parse_method(method)?;
    let body = match data {
        Some(raw) => Some(serde_json::from_str::<serde_json::Value>(&raw)?),
        None => None,
    };
    let value = client.raw(method, path, body).await?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn parse_method(s: &str) -> Result<reqwest::Method, CliError> {
    match s.to_ascii_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "PATCH" => Ok(reqwest::Method::PATCH),
        "PUT" => Ok(reqwest::Method::PUT),
        "DELETE" => Ok(reqwest::Method::DELETE),
        other => Err(CliError::InvalidArg(format!(
            "unsupported method `{other}`: expected GET|POST|PATCH|PUT|DELETE"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_method;

    #[test]
    fn parse_method_is_case_insensitive() {
        assert_eq!(parse_method("get").unwrap(), reqwest::Method::GET);
        assert_eq!(parse_method("Patch").unwrap(), reqwest::Method::PATCH);
        assert_eq!(parse_method("DELETE").unwrap(), reqwest::Method::DELETE);
    }

    #[test]
    fn parse_method_rejects_unknown() {
        let err = parse_method("FETCH").unwrap_err();
        assert!(err.to_string().contains("unsupported method"));
    }
}
