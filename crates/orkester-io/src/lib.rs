//! Async I/O traits for orkester.
//!
//! Provides the [`AssetAccessor`] trait for abstracting network/file I/O
//! in format-specific tile loaders, plus shared utilities used by all loaders.

use orkester::Task;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

pub use kiban::resolve_url;

/// Content-encoding of the raw bytes in an [`AssetResponse`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContentEncoding {
    /// Data is already in its final (uncompressed) form.
    #[default]
    None,
    /// Data is gzip-compressed and must be decompressed before use.
    Gzip,
}

/// Response from an asset request.
#[derive(Debug)]
pub struct AssetResponse {
    /// HTTP status code (or equivalent for non-HTTP sources).
    pub status: u16,
    /// Response body (may be compressed — see [`content_encoding`](AssetResponse::content_encoding)).
    pub data: Vec<u8>,
    /// Content encoding of [`data`](AssetResponse::data).
    pub content_encoding: ContentEncoding,
}

impl AssetResponse {
    /// Returns `Ok(())` for 2xx status codes, `Err(status)` otherwise.
    #[inline]
    pub fn check_status(&self) -> Result<(), u16> {
        if self.status >= 200 && self.status < 300 {
            Ok(())
        } else {
            Err(self.status)
        }
    }

    /// Borrow the response bytes, asserting they are already decompressed.
    ///
    /// In debug builds this panics if `content_encoding != None`.  Wrap the
    /// accessor with [`GunzipAccessor`] to satisfy the contract automatically.
    #[inline]
    pub fn decompressed_data(&self) -> &[u8] {
        debug_assert_eq!(
            self.content_encoding,
            ContentEncoding::None,
            "AssetResponse::decompressed_data called on a gzip-compressed \
             response — wrap your accessor with GunzipAccessor first"
        );
        &self.data
    }

    /// Consume the response and return the raw bytes, asserting they are
    /// already decompressed.
    ///
    /// In debug builds this panics if `content_encoding != None`.  Wrap the
    /// accessor with [`GunzipAccessor`] to satisfy the contract automatically.
    #[inline]
    pub fn into_decompressed_data(self) -> Vec<u8> {
        debug_assert_eq!(
            self.content_encoding,
            ContentEncoding::None,
            "AssetResponse::into_decompressed_data called on a gzip-compressed \
             response — wrap your accessor with GunzipAccessor first"
        );
        self.data
    }
}

/// Fetch priority hint passed to [`AssetAccessor::get`].
///
/// Implementations may use this to prioritise HTTP/2 stream weights,
/// reorder a download queue, or ignore it entirely. Higher values = higher
/// priority. The mapping from format-specific `LoadPriority` to this value
/// is the caller's responsibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestPriority(pub u8);

impl RequestPriority {
    pub const LOW: Self = Self(0);
    pub const NORMAL: Self = Self(128);
    pub const HIGH: Self = Self(255);
}

/// Async asset accessor for fetching tile data from network, file, or cache.
///
/// Implementations handle the actual I/O (HTTP, file system, SLPK archive, etc.).
/// Used by format-specific `ContentLoader` implementations to fetch tile content.
pub trait AssetAccessor: Send + Sync + 'static {
    /// Fetch the asset at the given URI.
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        priority: RequestPriority,
    ) -> Task<Result<AssetResponse, io::Error>>;

    /// Fetch the asset at the given URI with a byte range.
    fn get_range(
        &self,
        url: &str,
        headers: &[(String, String)],
        priority: RequestPriority,
        offset: u64,
        length: u64,
    ) -> Task<Result<AssetResponse, io::Error>>;
}

// ---------------------------------------------------------------------------
// GunzipAccessor
// ---------------------------------------------------------------------------

/// Decorator that decompresses gzip-encoded responses from an inner accessor.
///
/// If the response carries `ContentEncoding::Gzip` the data is decompressed
/// in the worker thread before the caller sees it. The resulting response has
/// `content_encoding: ContentEncoding::None`.
pub struct GunzipAccessor<A> {
    inner: A,
}

impl<A: AssetAccessor> GunzipAccessor<A> {
    pub fn new(inner: A) -> Self {
        Self { inner }
    }
}

fn gunzip_response(mut resp: AssetResponse) -> Result<AssetResponse, io::Error> {
    if resp.content_encoding == ContentEncoding::Gzip {
        use flate2::read::GzDecoder;
        use io::Read;
        let mut decoder = GzDecoder::new(resp.data.as_slice());
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        resp.data = decompressed;
        resp.content_encoding = ContentEncoding::None;
    }
    Ok(resp)
}

impl<A: AssetAccessor> AssetAccessor for GunzipAccessor<A> {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        priority: RequestPriority,
    ) -> Task<Result<AssetResponse, io::Error>> {
        self.inner
            .get(url, headers, priority)
            .map(|result| result.and_then(gunzip_response))
    }

    fn get_range(
        &self,
        url: &str,
        headers: &[(String, String)],
        priority: RequestPriority,
        offset: u64,
        length: u64,
    ) -> Task<Result<AssetResponse, io::Error>> {
        self.inner
            .get_range(url, headers, priority, offset, length)
            .map(|result| result.and_then(gunzip_response))
    }
}

// ---------------------------------------------------------------------------
// ArchiveAccessor
// ---------------------------------------------------------------------------

/// Self-contained accessor for `.zip`, `.slpk`, and `.3tz` local archives.
///
/// The ZIP central directory is read once at construction; thereafter each
/// lookup is O(1) (hash map). Outer archive entries are expected to be in
/// STORE mode; entries whose names end in `.gz` are decompressed with gzip
/// after extraction.
/// Self-contained accessor for `.zip`, `.slpk`, and `.3tz` local archives.
///
/// The ZIP central directory is read once at construction; thereafter each
/// lookup is O(1) (hash map). Outer archive entries are expected to be in
/// STORE mode; entries whose names end in `.gz` are decompressed with gzip
/// after extraction.
pub struct ArchiveAccessor {
    /// Maps entry name → compression method.
    index: Arc<HashMap<String, zip::CompressionMethod>>,
    /// The raw archive bytes. `Arc<[u8]>` avoids the extra Vec indirection.
    data: Arc<[u8]>,
    /// Runtime used to run blocking I/O on a background thread.
    bg_context: orkester::Context,
}

impl ArchiveAccessor {
    /// Open a local archive file (`.zip`, `.slpk`, `.3tz`).
    ///
    /// Reads and indexes the central directory synchronously. Returns an error
    /// if the file cannot be read or does not look like a valid ZIP archive.
    pub fn open(path: impl AsRef<Path>, bg_context: orkester::Context) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data, bg_context)
    }

    /// Create from an already-loaded byte buffer.
    pub fn from_bytes(data: Vec<u8>, bg_context: orkester::Context) -> io::Result<Self> {
        let cursor = io::Cursor::new(&data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut index = HashMap::with_capacity(archive.len());
        for i in 0..archive.len() {
            let file = archive
                .by_index_raw(i)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            index.insert(file.name().to_owned(), file.compression());
        }

        Ok(Self {
            index: Arc::new(index),
            data: Arc::from(data.into_boxed_slice()),
            bg_context,
        })
    }

    fn read_entry_sync(
        data: &[u8],
        index: &HashMap<String, zip::CompressionMethod>,
        name: &str,
    ) -> io::Result<Vec<u8>> {
        // Strip leading slash that may appear in "archive-relative" URLs.
        let key = name.trim_start_matches('/');

        let method = *index.get(key).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("entry not found: {key}"))
        })?;

        let cursor = io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut file = archive
            .by_name(key)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut buf = Vec::with_capacity(file.size() as usize);
        use io::Read;
        file.read_to_end(&mut buf)?;

        // Per-entry gzip decompression for `.gz`-named entries in STORE mode archives.
        let bytes = if key.ends_with(".gz") && method == zip::CompressionMethod::Stored {
            use flate2::read::GzDecoder;
            let mut dec = GzDecoder::new(buf.as_slice());
            let mut out = Vec::new();
            dec.read_to_end(&mut out)?;
            out
        } else {
            buf
        };

        Ok(bytes)
    }
}

fn archive_url_path(url: &str) -> &str {
    if let Some(pos) = url.find("://") {
        let after_scheme = &url[pos + 3..];
        after_scheme
            .find('/')
            .map_or(after_scheme, |i| &after_scheme[i..])
    } else {
        url
    }
}

impl AssetAccessor for ArchiveAccessor {
    fn get(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _priority: RequestPriority,
    ) -> Task<Result<AssetResponse, io::Error>> {
        let path = archive_url_path(url).to_owned();
        let data = Arc::clone(&self.data);
        let index = Arc::clone(&self.index);
        self.bg_context.run( move || {
            Self::read_entry_sync(&data, &index, &path).map(|data| AssetResponse {
                status: 200,
                data,
                content_encoding: ContentEncoding::None,
            })
        })
    }

    fn get_range(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _priority: RequestPriority,
        offset: u64,
        length: u64,
    ) -> Task<Result<AssetResponse, io::Error>> {
        let path = archive_url_path(url).to_owned();
        let data = Arc::clone(&self.data);
        let index = Arc::clone(&self.index);
        self.bg_context.run( move || {
            Self::read_entry_sync(&data, &index, &path).and_then(|entry_data| {
                let start = offset as usize;
                let end = (offset + length) as usize;
                if end > entry_data.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!(
                            "get_range out-of-bounds: offset={offset} length={length} entry_len={}",
                            entry_data.len()
                        ),
                    ));
                }
                Ok(AssetResponse {
                    status: 206,
                    data: entry_data[start..end].to_vec(),
                    content_encoding: ContentEncoding::None,
                })
            })
        })
    }
}


/// Synchronous `file://` accessor for local filesystem paths.
///
/// Resolves `file:///absolute/path` or bare `absolute/path` URIs by reading
/// the file from disk on a background worker thread. Useful for tests and
/// offline tooling.
///
/// Query strings and fragments are stripped before resolving the path.
pub struct FileAccessor {
    bg_context: orkester::Context,
}

impl FileAccessor {
    pub fn new(bg_context: orkester::Context) -> Self {
        Self { bg_context }
    }

    fn url_to_path(url: &str) -> std::path::PathBuf {
        // Strip scheme: file:// or file:///
        let stripped = if let Some(rest) = url.strip_prefix("file:///") {
            rest
        } else if let Some(rest) = url.strip_prefix("file://") {
            rest
        } else {
            url
        };
        // Strip query and fragment.
        let stripped = stripped.split('?').next().unwrap_or(stripped);
        let stripped = stripped.split('#').next().unwrap_or(stripped);
        std::path::PathBuf::from(stripped)
    }
}

impl AssetAccessor for FileAccessor {
    fn get(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _priority: RequestPriority,
    ) -> orkester::Task<Result<AssetResponse, io::Error>> {
        let path = Self::url_to_path(url);
        self.bg_context.run( move || {
            std::fs::read(&path).map(|data| AssetResponse {
                status: 200,
                data,
                content_encoding: ContentEncoding::None,
            })
        })
    }

    fn get_range(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _priority: RequestPriority,
        offset: u64,
        length: u64,
    ) -> orkester::Task<Result<AssetResponse, io::Error>> {
        debug_assert!(length > 0, "get_range called with length=0");
        // Guard against OOM from an unreasonably large length.
        const MAX_FILE_RANGE: u64 = 512 * 1024 * 1024; // 512 MiB
        if length > MAX_FILE_RANGE {
            let msg = format!("get_range: requested length {length} exceeds limit ({MAX_FILE_RANGE})");
            return orkester::resolved(Err(io::Error::new(io::ErrorKind::InvalidInput, msg)));
        }
        let path = Self::url_to_path(url);
        self.bg_context.run( move || {
            use io::Read as _;
            let mut file = std::fs::File::open(&path)?;
            io::Seek::seek(&mut file, io::SeekFrom::Start(offset))?;
            let mut buf = vec![0u8; length as usize];
            let n = file.read(&mut buf)?;
            buf.truncate(n);
            Ok(AssetResponse {
                status: 206,
                data: buf,
                content_encoding: ContentEncoding::None,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// HttpAccessor
// ---------------------------------------------------------------------------

/// Blocking HTTP/HTTPS accessor backed by [`ureq`], dispatched on background workers.
///
/// Each call to [`get`](AssetAccessor::get) or [`get_range`](AssetAccessor::get_range)
/// issues a `ureq` request on the orkester background thread pool so the main
/// thread is never blocked. Gzip responses are decompressed automatically.
///
/// # Default headers
/// Headers set at construction (e.g. `Authorization`) are merged with
/// per-request headers; per-request headers take precedence.
pub struct HttpAccessor {
    bg_context: orkester::Context,
    /// Shared, immutable headers applied to every request. `Arc` makes cloning O(1).
    default_headers: Arc<[(String, String)]>,
}

impl HttpAccessor {
    /// Create an accessor with no default headers.
    pub fn new(bg_context: orkester::Context) -> Self {
        Self {
            bg_context,
            default_headers: Arc::from([]),
        }
    }

    /// Create an accessor with default headers applied to every request.
    ///
    /// Useful for `Authorization`, `X-Api-Key`, and similar static headers.
    pub fn with_headers(
        bg_context: orkester::Context,
        headers: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        Self {
            bg_context,
            default_headers: headers.into_iter().collect(),
        }
    }

    fn build_request(
        url: &str,
        default_headers: &[(String, String)],
        extra_headers: &[(String, String)],
    ) -> ureq::Request {
        let mut req = ureq::get(url);
        for (k, v) in default_headers.iter().chain(extra_headers.iter()) {
            req = req.set(k, v);
        }
        req
    }

    fn read_response(response: ureq::Response) -> Result<AssetResponse, io::Error> {
        use io::Read;
        let status = response.status();
        let mut buf = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(AssetResponse {
            status,
            data: buf,
            content_encoding: ContentEncoding::None,
        })
    }

    fn map_ureq_error(e: ureq::Error) -> io::Error {
        match e {
            ureq::Error::Status(code, resp) => io::Error::new(
                io::ErrorKind::Other,
                format!("HTTP {code}: {}", resp.status_text()),
            ),
            ureq::Error::Transport(t) => io::Error::new(io::ErrorKind::Other, t),
        }
    }
}

impl AssetAccessor for HttpAccessor {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        _priority: RequestPriority,
    ) -> Task<Result<AssetResponse, io::Error>> {
        let url = url.to_owned();
        let default_headers = Arc::clone(&self.default_headers);
        let extra_headers = headers.to_vec();
        self.bg_context.run( move || {
            HttpAccessor::build_request(&url, &default_headers, &extra_headers)
                .call()
                .map_err(HttpAccessor::map_ureq_error)
                .and_then(HttpAccessor::read_response)
        })
    }

    fn get_range(
        &self,
        url: &str,
        headers: &[(String, String)],
        _priority: RequestPriority,
        offset: u64,
        length: u64,
    ) -> Task<Result<AssetResponse, io::Error>> {
        let url = url.to_owned();
        let default_headers = Arc::clone(&self.default_headers);
        let mut range_header = headers.to_vec();
        // RFC 7233: bytes=<first>-<last> (inclusive)
        range_header.push(("Range".to_owned(), {
            // RFC 7233: bytes=<first>-<last> (inclusive).
            // Guard against overflow when offset and length are both near u64::MAX.
            let last = offset
                .checked_add(length)
                .and_then(|e| e.checked_sub(1))
                .unwrap_or(u64::MAX);
            format!("bytes={offset}-{last}")
        }));
        let extra_headers = range_header;
        self.bg_context.run( move || {
            HttpAccessor::build_request(&url, &default_headers, &extra_headers)
                .call()
                .map_err(HttpAccessor::map_ureq_error)
                .and_then(HttpAccessor::read_response)
        })
    }
}
