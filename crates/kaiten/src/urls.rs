//! Syntax check for user-supplied links. Kaiten stores any string as an
//! external link url, so the CLI and the MCP server refuse obvious garbage
//! before sending it.

/// Trims `raw` and checks that it is an absolute `http(s)` URL without
/// embedded credentials. Returns the trimmed text as typed (no
/// normalization) so the API stores what the user wrote. The error never
/// echoes the value — it may contain a secret.
pub(crate) fn absolute_http_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("is empty".into());
    }
    // The WHATWG parser strips control characters and repairs `http:x`,
    // `http:/x` and `http:///x` into `http://x/`, but Kaiten stores the raw
    // text — so the text itself is checked, not only its parsed form: an
    // `http(s)://` prefix, a host right after it, no control characters.
    let after_scheme = ["http://", "https://"].iter().find_map(|prefix| {
        trimmed
            .get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| &trimmed[prefix.len()..])
    });
    let Some(authority) = after_scheme else {
        return Err("is not an absolute http(s) URL".into());
    };
    if authority.starts_with(['/', '\\']) || trimmed.chars().any(char::is_control) {
        return Err("is not an absolute http(s) URL".into());
    }
    let parsed =
        url::Url::parse(trimmed).map_err(|_| String::from("is not an absolute http(s) URL"))?;
    if parsed.host_str().is_none() {
        return Err("is not an absolute http(s) URL".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("must not contain credentials".into());
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::absolute_http_url;

    /// Surrounding whitespace (including a trailing newline from a shell)
    /// is trimmed; only interior control characters are refused.
    #[test]
    fn accepts_http_and_https_and_trims() {
        assert_eq!(
            absolute_http_url("  https://example.com/a?b=1#c  ").unwrap(),
            "https://example.com/a?b=1#c"
        );
        assert_eq!(
            absolute_http_url("http://host:8080/x").unwrap(),
            "http://host:8080/x"
        );
        assert_eq!(
            absolute_http_url("HTTPS://Example.com/x\r\n").unwrap(),
            "HTTPS://Example.com/x"
        );
    }

    #[test]
    fn rejects_garbage_without_echoing_it() {
        assert_eq!(absolute_http_url("").unwrap_err(), "is empty");
        assert_eq!(absolute_http_url("   ").unwrap_err(), "is empty");
        for bad in [
            "notaurl",
            "example.com/x",
            "ftp://host/x",
            "mailto:a@b",
            "http:",
            // the WHATWG parser would "repair" these, but Kaiten stores the raw text
            "http:x",
            "http:/x",
            "http:///x",
            "http://\\host/x",
            "HTTP://ho\rst/x",
            "https://exam\nple.com/x",
            "https://exa\tmple.com/x",
            "https://host/x\0",
        ] {
            match absolute_http_url(bad) {
                Ok(accepted) => panic!("{bad:?} was accepted as {accepted:?}"),
                Err(err) => assert_eq!(err, "is not an absolute http(s) URL", "{bad:?}"),
            }
        }
    }

    #[test]
    fn rejects_credentials_without_echoing_them() {
        let err = absolute_http_url("https://user:s3cret@host/x").unwrap_err();
        assert_eq!(err, "must not contain credentials");
        assert!(!err.contains("s3cret"));
    }
}
