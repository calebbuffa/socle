//! Simple URI / URL resolution utilities.
//!
//! Mirrors the behaviour of [`CesiumUtility::Uri`] for the cases needed by
//! i3s-native:
//!
//! * Absolute URIs (contain `://`) are returned unchanged.
//! * Scheme-relative URIs (start with `//`) inherit the scheme of the base.
//! * Absolute-path references (start with `/`) replace the path of the base
//!   origin.
//! * Relative-path references are resolved against the directory part of the
//!   base path (the last path segment is removed before appending).
//!
//! Query strings and fragments are preserved as-is.  No percent-encoding or
//! decoding is performed beyond what the caller provides.

/// Resolve `relative` against `base`, returning an owned `String`.
///
/// # Examples
/// ```
/// use i3s_util::uri::resolve;
///
/// // Absolute URI is returned unchanged
/// assert_eq!(resolve("https://example.com/a/b", "https://other.com/c"), "https://other.com/c");
///
/// // Scheme-relative
/// assert_eq!(resolve("https://example.com/a/b", "//cdn.example.com/img.png"),
///            "https://cdn.example.com/img.png");
///
/// // Absolute path
/// assert_eq!(resolve("https://example.com/a/b/c", "/root/d"), "https://example.com/root/d");
///
/// // Relative path
/// assert_eq!(resolve("https://example.com/a/b/layer.json", "nodes/0"),
///            "https://example.com/a/b/nodes/0");
/// ```
pub fn resolve(base: &str, relative: &str) -> String {
    // 1. Absolute URI
    if relative.contains("://") {
        return relative.to_owned();
    }

    // 2. Scheme-relative (protocol-relative) reference
    if relative.starts_with("//") {
        let scheme = base.split("://").next().unwrap_or("https");
        return format!("{scheme}:{relative}");
    }

    // 3. Absolute-path reference
    if relative.starts_with('/') {
        let origin = origin_of(base);
        return format!("{origin}{relative}");
    }

    // 4. Relative-path reference — resolve against the directory of base
    let dir = dir_of(base);
    if dir.ends_with('/') {
        format!("{dir}{relative}")
    } else {
        // dir_of returned the full base with no trailing slash (base has no path)
        format!("{dir}/{relative}")
    }
}

/// Returns the origin (`scheme://host:port`) portion of a URL, or an empty
/// string if the URL has no recognised scheme.
pub fn origin_of(url: &str) -> &str {
    // Find "://" separator and then the first '/' after it (path start)
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(path_start) = after_scheme.find('/') {
            return &url[..scheme_end + 3 + path_start];
        }
        // No path → the whole URL is the origin
        return url;
    }
    ""
}

/// Returns the directory part of a URL up to and including the last `/` that
/// follows the authority (host) portion.
///
/// For example `"https://example.com/a/b/file.json"` → `"https://example.com/a/b/"`.
/// For `"https://example.com"` (no path) returns the full URL unchanged — the
/// caller is responsible for adding `/` if needed.
pub fn dir_of(url: &str) -> &str {
    // Skip past "://" so we don't pick up the slashes in the scheme.
    let search_from = url.find("://").map(|i| i + 3).unwrap_or(0);
    if let Some(pos) = url[search_from..].rfind('/') {
        return &url[..search_from + pos + 1];
    }
    // No path separator after the authority.
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_passthrough() {
        assert_eq!(
            resolve("https://example.com/a", "https://other.com/b"),
            "https://other.com/b"
        );
    }

    #[test]
    fn scheme_relative() {
        assert_eq!(
            resolve("https://example.com/a", "//cdn.net/img.png"),
            "https://cdn.net/img.png"
        );
    }

    #[test]
    fn absolute_path() {
        assert_eq!(
            resolve("https://example.com/a/b/c", "/root/d"),
            "https://example.com/root/d"
        );
    }

    #[test]
    fn relative_path() {
        assert_eq!(
            resolve("https://example.com/a/b/layer.json", "nodes/0"),
            "https://example.com/a/b/nodes/0"
        );
    }

    #[test]
    fn relative_path_no_path_in_base() {
        assert_eq!(
            resolve("https://example.com", "layer.json"),
            "https://example.com/layer.json"
        );
    }

    #[test]
    fn origin_of_normal() {
        assert_eq!(origin_of("https://example.com/a/b"), "https://example.com");
    }

    #[test]
    fn dir_of_normal() {
        assert_eq!(
            dir_of("https://example.com/a/b/file.json"),
            "https://example.com/a/b/"
        );
    }
}
