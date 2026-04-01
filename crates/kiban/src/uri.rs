//! URI and file-path utilities.

use std::fmt;

/// An owned, resolved URI string.
///
/// Wraps a `String` but makes the intent explicit and provides common
/// trait implementations for use as map keys, debug output, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uri(pub(crate) String);

impl Uri {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn resolve(&self, key: &str) -> String {
        resolve_url(&self.0, key)
    }

    pub fn extension(&self) -> Option<&str> {
        file_extension(&self.0)
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for Uri {
    fn from(s: String) -> Self {
        Uri(s)
    }
}

impl From<&str> for Uri {
    fn from(s: &str) -> Self {
        Uri(s.to_owned())
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Resolve `key` relative to `base`.
///
/// If `key` is already absolute (starts with a scheme such as `https://` or
/// `file://`, or with `/` or `\`) it is returned as-is. Otherwise it is
/// appended to the directory portion of `base` (everything up to and including
/// the last `/` or `\`, with query and fragment stripped).
///
/// Handles both Unix `/` and Windows `\` path separators.
///
/// # Examples
///
/// ```
/// use kiban::resolve_url;
///
/// assert_eq!(
///     resolve_url("https://example.com/tiles/model.gltf", "buffer0.bin"),
///     "https://example.com/tiles/buffer0.bin"
/// );
/// assert_eq!(
///     resolve_url(r"C:\tiles\model.gltf", "tex.png"),
///     r"C:\tiles\tex.png"
/// );
/// assert_eq!(
///     resolve_url("https://example.com/a.gltf", "/root/b.png"),
///     "/root/b.png"
/// );
/// ```
pub fn resolve_url(base: &str, key: &str) -> String {
    // Already absolute: has a scheme, starts with /, \, or is a Windows drive-letter path (e.g. C:\).
    let is_absolute = key.contains("://")
        || key.starts_with('/')
        || key.starts_with('\\')
        || (key.len() >= 2 && key.as_bytes()[1] == b':' && key.as_bytes()[0].is_ascii_alphabetic());
    if is_absolute {
        return key.to_owned();
    }
    let base_path = base.split('?').next().unwrap_or(base);
    let base_path = base_path.split('#').next().unwrap_or(base_path);
    let last_sep = base_path.rfind(|c| c == '/' || c == '\\');
    let dir = last_sep.map_or("", |i| &base_path[..=i]);
    format!("{dir}{key}")
}

/// Extract the file extension from a URL or file path.
///
/// Strips any query string (`?…`) and fragment (`#…`) before looking for the
/// last `.`.  Returns `None` if the path has no extension or ends with a
/// separator.  The returned slice preserves the original casing — compare with
/// [`str::eq_ignore_ascii_case`] or call `.to_ascii_lowercase()` yourself.
///
/// # Examples
///
/// ```
/// use kiban::file_extension;
///
/// assert_eq!(file_extension("https://example.com/data/tile.b3dm?v=1"), Some("b3dm"));
/// assert!(file_extension("model.GLB").map_or(false, |e| e.eq_ignore_ascii_case("glb")));
/// assert_eq!(file_extension("no_extension"), None);
/// ```
pub fn file_extension(url: &str) -> Option<&str> {
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);
    let last_sep = path.rfind(|c| c == '/' || c == '\\').map_or(0, |i| i + 1);
    let filename = &path[last_sep..];
    let dot = filename.rfind('.')?;
    let ext = &filename[dot + 1..];
    if ext.is_empty() {
        return None;
    }
    // Return a ref into the original slice without allocating; callers that
    // need lowercase must call `.to_ascii_lowercase()` themselves.  We return
    // a sub-slice of the input so the lifetime is tied to `url`.
    //
    // NOTE: ASCII-case-fold without allocation — we return the raw slice and
    // document that extension comparisons should use `eq_ignore_ascii_case`.
    Some(ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_http() {
        assert_eq!(
            resolve_url("https://example.com/tiles/model.gltf", "buffer0.bin"),
            "https://example.com/tiles/buffer0.bin"
        );
    }

    #[test]
    fn resolve_relative_file() {
        assert_eq!(
            resolve_url("/data/tiles/model.gltf", "textures/tex.png"),
            "/data/tiles/textures/tex.png"
        );
    }

    #[test]
    fn resolve_windows_path() {
        assert_eq!(
            resolve_url(
                r"C:\Users\foo\tiles/data/Models\Box\glTF\Box.gltf",
                "Box0.bin"
            ),
            r"C:\Users\foo\tiles/data/Models\Box\glTF\Box0.bin"
        );
    }

    #[test]
    fn resolve_absolute_key_passthrough() {
        assert_eq!(
            resolve_url(
                "https://example.com/tiles/model.gltf",
                "https://cdn.example.com/buf.bin"
            ),
            "https://cdn.example.com/buf.bin"
        );
    }

    #[test]
    fn resolve_root_relative() {
        assert_eq!(
            resolve_url("https://example.com/tiles/model.gltf", "/other/tex.png"),
            "/other/tex.png"
        );
    }

    #[test]
    fn resolve_strips_query_from_base() {
        assert_eq!(
            resolve_url(
                "https://example.com/tiles/model.gltf?token=abc",
                "buffer0.bin"
            ),
            "https://example.com/tiles/buffer0.bin"
        );
    }

    #[test]
    fn ext_http_with_query() {
        assert_eq!(
            file_extension("https://example.com/data/tile.b3dm?v=1"),
            Some("b3dm")
        );
    }

    #[test]
    fn ext_preserves_case() {
        assert_eq!(file_extension("model.GLB"), Some("GLB"));
    }

    #[test]
    fn ext_no_extension() {
        assert_eq!(file_extension("no_extension"), None);
    }

    #[test]
    fn ext_trailing_dot() {
        assert_eq!(file_extension("trailing."), None);
    }
}
