pub fn sanitize_diagnostic_text(value: &str, max_chars: usize) -> String {
    let mut sanitized = redact_key_value(
        value,
        &[
            "api_key",
            "apiKey",
            "token",
            "secret",
            "transcript",
            "providerBody",
            "petText",
        ],
    );
    sanitized = redact_bearer(&sanitized);
    sanitized = sanitized
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ");
    sanitized.chars().take(max_chars).collect()
}

fn redact_key_value(value: &str, keys: &[&str]) -> String {
    let mut output = value.to_string();
    for key in keys {
        for separator in ['=', ':'] {
            let marker = format!("{key}{separator}");
            let mut cursor = 0;
            while let Some(offset) = output[cursor..].find(&marker) {
                let start = cursor + offset + marker.len();
                let end = output[start..]
                    .find(|character: char| {
                        character.is_whitespace() || matches!(character, ',' | ';' | '}' | ']')
                    })
                    .map(|offset| start + offset)
                    .unwrap_or(output.len());
                output.replace_range(start..end, "[redacted]");
                cursor = start + "[redacted]".len();
            }
        }
    }
    output
}

fn redact_bearer(value: &str) -> String {
    let mut output = value.to_string();
    let mut cursor = 0;
    while let Some(offset) = output[cursor..].to_ascii_lowercase().find("bearer ") {
        let secret_start = cursor + offset + "bearer ".len();
        let secret_end = output[secret_start..]
            .find(char::is_whitespace)
            .map(|end| secret_start + end)
            .unwrap_or(output.len());
        output.replace_range(secret_start..secret_end, "[redacted]");
        cursor = secret_start + "[redacted]".len();
    }
    output
}

fn redact_token(token: &str) -> String {
    let trimmed = token
        .trim_matches(|character| matches!(character, '\'' | '"' | '(' | '[' | '{' | ',' | ';'));
    let unix_path = trimmed.starts_with('/');
    let windows_path = trimmed.as_bytes().get(1) == Some(&b':')
        && trimmed
            .as_bytes()
            .get(2)
            .is_some_and(|byte| matches!(byte, b'\\' | b'/'));
    if unix_path || windows_path {
        return "[redacted-path]".into();
    }

    if let Some(scheme_offset) = trimmed.find("://") {
        let authority_start = scheme_offset + 3;
        let path_start = trimmed[authority_start..]
            .find('/')
            .map(|offset| authority_start + offset)
            .unwrap_or(trimmed.len());
        let authority = &trimmed[authority_start..path_start];
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let path = &trimmed[path_start..];
        let safe_path = path.split(['?', '#']).next().unwrap_or(path);
        return format!("{}://{}{}", &trimmed[..scheme_offset], host, safe_path);
    }

    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_credentials_urls_paths_and_sensitive_content() {
        let output = sanitize_diagnostic_text(
            "api_key=secret Bearer token123 https://user:pass@example.com/v1?token=x /Users/me/audio.wav transcript=private",
            4096,
        );
        for sensitive in [
            "secret",
            "token123",
            "user:pass",
            "?token=x",
            "/Users/me",
            "private",
        ] {
            assert!(!output.contains(sensitive), "leaked {sensitive}: {output}");
        }
        assert!(output.contains("https://example.com/v1"));
        assert!(output.contains("[redacted-path]"));
    }
}
