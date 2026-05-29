//! URL validation + normalization.
//!
//! Returns the *normalized* URL string on success, or `AppError::BadRequest` so the
//! handler can `?`-propagate it straight to a 400.

use url::Url;

use crate::error::AppError;

const MAX_URL_LENGTH: usize = 2048;

/// Demo blocklist — short links must not become phishing/malware vectors.
const BLOCKED_DOMAINS: [&str; 3] = ["evil.com", "malware.example.com", "phishing.example.com"];

/// Validate and normalize a user-supplied URL.
///
/// Normalization rules:
///   * scheme + host are lowercased (the `url` crate does this on parse — DNS and
///     schemes are case-insensitive),
///   * the path/query are left untouched (they ARE case-sensitive),
///   * a single trailing `/` is stripped so `https://x.com/` == `https://x.com`.
///
/// We deliberately do NOT force http->https (that would change the user's intended
/// target) and do NOT append a trailing slash.
pub fn validate_url(input: &str) -> Result<String, AppError> {
    let input = input.trim();

    if input.len() > MAX_URL_LENGTH {
        return Err(AppError::BadRequest("URL exceeds max length".into()));
    }

    let parsed =
        Url::parse(input).map_err(|_| AppError::BadRequest("Invalid URL format".into()))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(AppError::BadRequest(format!("Invalid scheme: {other}"))),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::BadRequest("URL has no host".into()))?;

    if BLOCKED_DOMAINS.contains(&host.to_lowercase().as_str()) {
        return Err(AppError::BadRequest("URL is on the blocklist".into()));
    }

    // `as_str()` is the normalized serialization (lowercased scheme/host, default port
    // stripped). It always carries at least a "/" path, so drop a lone trailing slash.
    let mut normalized = parsed.as_str().to_string();
    if normalized.ends_with('/') {
        normalized.pop();
    }

    Ok(normalized)
}
