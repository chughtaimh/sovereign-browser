// Pure navigation logic - no Tauri imports allowed.
// This module contains URL parsing and navigation helpers that can be unit tested.

use url::Url;
use crate::settings::Settings;

/// Logic for parsing input into a navigable URL.
///
/// PRIVACY NOTICE:
/// This function performs purely local string manipulation and heuristics.
/// 1. It does NOT perform any DNS resolution or network reachability checks.
/// 2. It does NOT prefetch any content.
/// 3. It does NOT send any data to autocomplete servers.
/// 4. The only external request happens when the user explicitly commits navigation (Enter/Go),
///    at which point the Webview initiates a standard navigation.
pub fn smart_parse_url(input: &str, settings: &Settings) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }

    // 1. Force HTTP for implicit localhost/IP (if no scheme present)
    let has_scheme_separator = trimmed.contains("://");
    let is_localhost = trimmed.starts_with("localhost") || trimmed.starts_with("127.0.0.1");
    let is_ip = trimmed.parse::<std::net::IpAddr>().is_ok();

    if (is_localhost || is_ip) && !has_scheme_separator {
        let candidate = format!("http://{}", trimmed);
        if let Ok(u) = Url::parse(&candidate) {
            return u.to_string();
        }
    }

    // 2. Try parsing as-is (valid scheme)
    if let Ok(u) = Url::parse(trimmed) {
        let s = u.scheme();
        // Only accept if it's a known standard web/file scheme
        // This prevents "google.com" being parsed as scheme "google"
        if s == "http" || s == "https" || s == "file" || s == "about" || s == "data" {
            return u.to_string();
        }
    }

    // 3. Heuristic: Dot implies domain? -> Try HTTPS (or HTTP if https_only is false)
    // (Exclude spaces which imply search)
    if !trimmed.contains(' ') && trimmed.contains('.') && !trimmed.ends_with('.') {
        let scheme = if settings.https_only { "https" } else { "http" };
        let candidate = format!("{}://{}", scheme, trimmed);
        if let Ok(u) = Url::parse(&candidate) {
            if u.host().is_some() {
                return u.to_string();
            }
        }
    }

    // 4. Fallback to configured Search Engine
    settings.search_engine.query_url(trimmed)
}

/// Guess the resource type based on URL extension (for adblock engine).
pub fn guess_request_type(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains(".js") || lower.contains("javascript") {
        "script".to_string()
    } else if lower.contains(".css") {
        "stylesheet".to_string()
    } else if lower.contains(".png")
        || lower.contains(".jpg")
        || lower.contains(".jpeg")
        || lower.contains(".gif")
        || lower.contains(".webp")
        || lower.contains(".svg")
        || lower.contains(".ico")
    {
        "image".to_string()
    } else if lower.contains(".woff") || lower.contains(".ttf") || lower.contains(".otf") {
        "font".to_string()
    } else if lower.contains(".mp4") || lower.contains(".webm") || lower.contains(".m3u8") {
        "media".to_string()
    } else if lower.contains("xmlhttprequest")
        || lower.contains("/api/")
        || lower.contains("/ajax/")
    {
        "xmlhttprequest".to_string()
    } else {
        "other".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, SearchEngine};
    use rstest::rstest;

    // --- smart_parse_url tests ---

    #[rstest]
    // Standard URLs (should remain unchanged or normalized)
    #[case("https://example.com", "https://example.com/")]
    #[case("http://example.com", "http://example.com/")]
    #[case("https://example.com/path?query=1", "https://example.com/path?query=1")]
    // Localhost handling (should get http://)
    #[case("localhost", "http://localhost/")]
    #[case("localhost:3000", "http://localhost:3000/")]
    #[case("localhost:8080/path", "http://localhost:8080/path")]
    // IP address handling (should get http://)
    #[case("127.0.0.1", "http://127.0.0.1/")]
    #[case("127.0.0.1:8080", "http://127.0.0.1:8080/")]
    #[case("192.168.1.1", "http://192.168.1.1/")]
    // Domain-like strings (should get https://)
    #[case("google.com", "https://google.com/")]
    #[case("sub.domain.com", "https://sub.domain.com/")]
    #[case("example.co.uk", "https://example.co.uk/")]
    #[case("docs.rs/my-crate", "https://docs.rs/my-crate")]
    // Special schemes
    #[case("about:blank", "about:blank")]
    #[case("file:///Users/test/doc.html", "file:///Users/test/doc.html")]
    #[case("data:text/html,<h1>Hello</h1>", "data:text/html,<h1>Hello</h1>")]
    // Edge cases
    #[case("", "about:blank")]
    #[case("   ", "about:blank")]
    fn test_smart_url_parsing(#[case] input: &str, #[case] expected: &str) {
        let settings = Settings::default(); // https_only = true by default
        assert_eq!(smart_parse_url(input, &settings), expected);
    }

    // Test search query fallback (spaces in input -> search)
    #[rstest]
    #[case("hello world", "https://duckduckgo.com/?q=hello%20world")]
    #[case("rust programming", "https://duckduckgo.com/?q=rust%20programming")]
    #[case("what is tauri", "https://duckduckgo.com/?q=what%20is%20tauri")]
    fn test_search_fallback(#[case] input: &str, #[case] expected: &str) {
        let settings = Settings::default();
        assert_eq!(smart_parse_url(input, &settings), expected);
    }

    // Test with different search engines
    #[test]
    fn test_google_search_engine() {
        let mut settings = Settings::default();
        settings.search_engine = SearchEngine::Google;
        assert_eq!(
            smart_parse_url("test query", &settings),
            "https://google.com/search?q=test%20query"
        );
    }

    #[test]
    fn test_https_only_off() {
        let mut settings = Settings::default();
        settings.https_only = false;
        // When https_only is false, domains should get http://
        assert_eq!(smart_parse_url("example.com", &settings), "http://example.com/");
    }

    // --- guess_request_type tests ---

    #[rstest]
    #[case("https://example.com/script.js", "script")]
    #[case("https://example.com/style.css", "stylesheet")]
    #[case("https://example.com/image.png", "image")]
    #[case("https://example.com/photo.jpg", "image")]
    #[case("https://example.com/icon.ico", "image")]
    #[case("https://example.com/font.woff2", "font")]
    #[case("https://example.com/video.mp4", "media")]
    #[case("https://example.com/api/data", "xmlhttprequest")]
    #[case("https://example.com/page.html", "other")]
    fn test_guess_request_type(#[case] url: &str, #[case] expected: &str) {
        assert_eq!(guess_request_type(url), expected);
    }
}
