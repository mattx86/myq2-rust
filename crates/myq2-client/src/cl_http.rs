// cl_http.rs -- HTTP download support (similar to R1Q2)
//
// Allows downloading game assets (maps, models, sounds, etc.) via HTTP
// instead of the slower in-game UDP protocol. Server provides a base URL
// via the "sv_downloadurl" cvar.
//
// Uses async (non-blocking) downloads that run in the background while the
// game continues. The game thread polls for progress updates and completion.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{CONTENT_LENGTH, RANGE};

use myq2_common::common::{com_printf, DISTNAME, DISTVER};

// =============================================================================
// Constants
// =============================================================================

/// Default timeout for HTTP connections (seconds)
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 30;

/// Default timeout for HTTP reads (seconds)
const HTTP_READ_TIMEOUT_SECS: u64 = 60;

// =============================================================================
// Types
// =============================================================================

/// Result of an HTTP download operation
#[derive(Debug)]
pub enum HttpDownloadResult {
    /// Download completed successfully
    Success,
    /// File not found on server (404)
    NotFound,
    /// Server doesn't support HTTP downloads
    NotAvailable,
    /// Download was cancelled
    Cancelled,
    /// Network or I/O error
    Error(String),
}

/// Progress information for download callbacks
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Name of the file being downloaded
    pub filename: String,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total file size (if known)
    pub total_bytes: Option<u64>,
    /// Download speed in bytes per second
    pub bytes_per_second: u64,
}

// =============================================================================
// Async HTTP Download System
// =============================================================================

use tokio::sync::mpsc;
use std::thread;

/// Status of an async download
#[derive(Debug, Clone)]
pub enum AsyncDownloadStatus {
    /// Download is in progress
    InProgress(DownloadProgress),
    /// Download completed successfully
    Completed,
    /// Download failed
    Failed(String),
    /// Download was cancelled
    Cancelled,
    /// File not found (404)
    NotFound,
}

/// A queued download request
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    /// Remote filename (relative path)
    pub filename: String,
    /// Local destination path
    pub dest_path: PathBuf,
    /// Unique ID for this download
    pub id: u64,
}

/// Async HTTP download manager
///
/// Runs downloads in a background thread pool, sending progress updates
/// via channels. The game thread can poll for updates without blocking.
pub struct AsyncHttpDownloadManager {
    /// Base URL for downloads
    base_url: String,
    /// Sender for download requests
    request_tx: mpsc::UnboundedSender<DownloadRequest>,
    /// Receiver for status updates
    status_rx: mpsc::UnboundedReceiver<(u64, AsyncDownloadStatus)>,
    /// Cancel flag shared with download tasks
    cancel_flag: Arc<AtomicBool>,
    /// Next download ID
    next_id: AtomicU64,
    /// Tokio runtime handle
    _runtime_handle: thread::JoinHandle<()>,
}

impl AsyncHttpDownloadManager {
    /// Create a new async download manager with the given base URL.
    ///
    /// Spawns a background thread running a tokio runtime for async I/O.
    pub fn new(base_url: &str) -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel::<DownloadRequest>();
        let (status_tx, status_rx) = mpsc::unbounded_channel::<(u64, AsyncDownloadStatus)>();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let base_url_clone = base_url.to_string();
        let cancel_flag_clone = Arc::clone(&cancel_flag);

        // Spawn background thread with tokio runtime
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async_download_loop(
                base_url_clone,
                request_rx,
                status_tx,
                cancel_flag_clone,
            ));
        });

        Self {
            base_url: base_url.to_string(),
            request_tx,
            status_rx,
            cancel_flag,
            next_id: AtomicU64::new(1),
            _runtime_handle: handle,
        }
    }

    /// Queue a file for download.
    ///
    /// Returns the download ID that can be used to track this download.
    /// The download starts immediately in the background.
    pub fn queue_download(&self, filename: &str, dest_path: &Path) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = DownloadRequest {
            filename: filename.to_string(),
            dest_path: dest_path.to_path_buf(),
            id,
        };

        if let Err(e) = self.request_tx.send(request) {
            com_printf(&format!("Failed to queue download: {}\n", e));
        }

        id
    }

    /// Poll for status updates from background downloads.
    ///
    /// Returns all available updates without blocking.
    /// Call this once per frame to process download progress.
    pub fn poll_updates(&mut self) -> Vec<(u64, AsyncDownloadStatus)> {
        let mut updates = Vec::new();
        while let Ok(update) = self.status_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    /// Cancel all ongoing downloads.
    pub fn cancel_all(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Reset cancel flag (call after cancel_all to allow new downloads).
    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Check if downloads are available (base URL is set).
    pub fn is_available(&self) -> bool {
        !self.base_url.is_empty()
    }
}

/// Background async download loop.
///
/// Runs in a dedicated thread with a tokio runtime.
async fn async_download_loop(
    base_url: String,
    mut request_rx: mpsc::UnboundedReceiver<DownloadRequest>,
    status_tx: mpsc::UnboundedSender<(u64, AsyncDownloadStatus)>,
    cancel_flag: Arc<AtomicBool>,
) {
    let user_agent = format!("{}/v{:.2}", DISTNAME, DISTVER);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HTTP_READ_TIMEOUT_SECS))
        .user_agent(&user_agent)
        .build()
        .expect("Failed to create async HTTP client");

    while let Some(request) = request_rx.recv().await {
        if cancel_flag.load(Ordering::SeqCst) {
            let _ = status_tx.send((request.id, AsyncDownloadStatus::Cancelled));
            continue;
        }

        let result = async_download_file(
            &client,
            &base_url,
            &request.filename,
            &request.dest_path,
            request.id,
            &status_tx,
            &cancel_flag,
        ).await;

        // Send final status
        let final_status = match result {
            Ok(()) => AsyncDownloadStatus::Completed,
            Err(AsyncDownloadError::NotFound) => AsyncDownloadStatus::NotFound,
            Err(AsyncDownloadError::Cancelled) => AsyncDownloadStatus::Cancelled,
            Err(AsyncDownloadError::Other(msg)) => AsyncDownloadStatus::Failed(msg),
        };
        let _ = status_tx.send((request.id, final_status));
    }
}

/// Async download error types
enum AsyncDownloadError {
    NotFound,
    Cancelled,
    Other(String),
}

/// Perform a single async file download.
async fn async_download_file(
    client: &reqwest::Client,
    base_url: &str,
    filename: &str,
    dest_path: &Path,
    download_id: u64,
    status_tx: &mpsc::UnboundedSender<(u64, AsyncDownloadStatus)>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), AsyncDownloadError> {
    use futures_util::StreamExt;

    // Build URL
    let base = base_url.trim_end_matches('/');
    let file = filename.trim_start_matches('/');
    let url = format!("{}/{}", base, file);

    // Check for existing partial download
    let temp_path = dest_path.with_extension("tmp");
    let resume_offset = temp_path.metadata().map(|m| m.len()).unwrap_or(0);

    // Build request
    let mut request = client.get(&url);
    if resume_offset > 0 {
        request = request.header(RANGE, format!("bytes={}-", resume_offset));
    }

    // Send request
    let response = request.send().await.map_err(|e| {
        if e.is_connect() {
            AsyncDownloadError::Other(format!("Connection failed: {}", e))
        } else if e.is_timeout() {
            AsyncDownloadError::Other("Connection timed out".to_string())
        } else {
            AsyncDownloadError::Other(format!("Request failed: {}", e))
        }
    })?;

    // Check status
    match response.status() {
        reqwest::StatusCode::OK | reqwest::StatusCode::PARTIAL_CONTENT => {}
        reqwest::StatusCode::NOT_FOUND => {
            return Err(AsyncDownloadError::NotFound);
        }
        reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {
            // Resume failed, delete temp and retry would be needed
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AsyncDownloadError::Other("Resume not supported".to_string()));
        }
        status => {
            return Err(AsyncDownloadError::Other(format!("HTTP error: {}", status)));
        }
    }

    // Get content length
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let total_size = content_length.map(|cl| cl + resume_offset);

    // Create parent directories
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            AsyncDownloadError::Other(format!("Failed to create directory: {}", e))
        })?;
    }

    // Open file for writing
    let mut file = if resume_offset > 0 {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&temp_path)
            .await
    } else {
        tokio::fs::File::create(&temp_path).await
    }.map_err(|e| AsyncDownloadError::Other(format!("Failed to create file: {}", e)))?;

    // Stream download with progress
    let mut bytes_downloaded = resume_offset;
    let start_time = std::time::Instant::now();
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        // Check cancellation
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(AsyncDownloadError::Cancelled);
        }

        let chunk = chunk_result.map_err(|e| {
            AsyncDownloadError::Other(format!("Download error: {}", e))
        })?;

        // Write chunk
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk).await.map_err(|e| {
            AsyncDownloadError::Other(format!("Write error: {}", e))
        })?;

        bytes_downloaded += chunk.len() as u64;

        // Send progress update
        let elapsed = start_time.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            ((bytes_downloaded - resume_offset) as f64 / elapsed) as u64
        } else {
            0
        };

        let progress = DownloadProgress {
            filename: filename.to_string(),
            bytes_downloaded,
            total_bytes: total_size,
            bytes_per_second: speed,
        };
        let _ = status_tx.send((download_id, AsyncDownloadStatus::InProgress(progress)));
    }

    // Flush and rename
    use tokio::io::AsyncWriteExt;
    file.flush().await.map_err(|e| {
        AsyncDownloadError::Other(format!("Flush error: {}", e))
    })?;
    drop(file);

    tokio::fs::rename(&temp_path, dest_path).await.map_err(|e| {
        AsyncDownloadError::Other(format!("Failed to rename file: {}", e))
    })?;

    Ok(())
}

// =============================================================================
// Global async download manager
// =============================================================================

use std::sync::OnceLock;

static ASYNC_HTTP_MANAGER: OnceLock<parking_lot::Mutex<Option<AsyncHttpDownloadManager>>> = OnceLock::new();

fn global_async_manager() -> &'static parking_lot::Mutex<Option<AsyncHttpDownloadManager>> {
    ASYNC_HTTP_MANAGER.get_or_init(|| parking_lot::Mutex::new(None))
}

/// Initialize HTTP downloads with the given base URL.
/// Creates a background download thread that persists until shutdown.
pub fn cl_http_init(base_url: &str) {
    if base_url.is_empty() {
        com_printf("HTTP downloads: disabled (no server URL)\n");
        *global_async_manager().lock() = None;
        return;
    }

    let manager = AsyncHttpDownloadManager::new(base_url);
    com_printf(&format!("HTTP downloads: enabled ({})\n", base_url));
    *global_async_manager().lock() = Some(manager);
}

/// Shutdown HTTP downloads (called on disconnect).
pub fn cl_http_shutdown() {
    if let Some(manager) = global_async_manager().lock().as_ref() {
        manager.cancel_all();
    }
    *global_async_manager().lock() = None;
}

/// Check if HTTP downloads are available.
pub fn cl_http_available() -> bool {
    global_async_manager()
        .lock()
        .as_ref()
        .map(|m| m.is_available())
        .unwrap_or(false)
}

/// Queue a file for async download.
/// Returns download ID for tracking, or None if not available.
pub fn cl_http_download(filename: &str, dest_path: &Path) -> Option<u64> {
    global_async_manager()
        .lock()
        .as_ref()
        .map(|m| m.queue_download(filename, dest_path))
}

/// Poll for async download status updates.
/// Returns all pending updates. Call once per frame.
pub fn cl_http_poll() -> Vec<(u64, AsyncDownloadStatus)> {
    global_async_manager()
        .lock()
        .as_mut()
        .map(|m| m.poll_updates())
        .unwrap_or_default()
}

/// Cancel all ongoing HTTP downloads.
pub fn cl_http_cancel() {
    if let Some(manager) = global_async_manager().lock().as_ref() {
        manager.cancel_all();
    }
}

// =============================================================================
// Console command
// =============================================================================

/// HTTP download console command handler
/// Usage: httpdownload <filename>
///
/// Queues an async download and reports the download ID.
pub fn cmd_http_download(args: &[&str]) {
    if args.is_empty() {
        com_printf("Usage: httpdownload <filename>\n");
        return;
    }

    let filename = args[0];

    if !cl_http_available() {
        com_printf("HTTP downloads not available (server doesn't provide download URL)\n");
        return;
    }

    // Determine destination path
    let dest = PathBuf::from(filename);

    match cl_http_download(filename, &dest) {
        Some(id) => {
            com_printf(&format!("HTTP: Queued download {} (id={})\n", filename, id));
            com_printf("Use cl_http_poll status updates to track progress.\n");
        }
        None => {
            com_printf("HTTP downloads not available\n");
        }
    }
}

// =============================================================================
// Helper functions for URL construction (used by async_download_file)
// =============================================================================

/// Build a download URL from a base URL and filename.
/// Strips trailing slashes from base and leading slashes from file.
pub fn build_download_url(base_url: &str, filename: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let file = filename.trim_start_matches('/');
    format!("{}/{}", base, file)
}

/// Sanitize a download filename to prevent directory traversal.
/// Returns None if the path is unsafe (contains ".." or starts with "/").
pub fn sanitize_download_path(filename: &str) -> Option<String> {
    // Reject absolute paths
    if filename.starts_with('/') || filename.starts_with('\\') {
        return None;
    }

    // Reject directory traversal
    let normalized = filename.replace('\\', "/");
    for component in normalized.split('/') {
        if component == ".." {
            return None;
        }
    }

    // Reject paths that contain drive letters (e.g. "C:")
    if filename.len() >= 2 && filename.as_bytes()[1] == b':' {
        return None;
    }

    Some(normalized)
}

/// Calculate download progress percentage.
/// Returns 0 if total_bytes is unknown or zero.
pub fn calculate_progress_percent(bytes_downloaded: u64, total_bytes: Option<u64>) -> i32 {
    match total_bytes {
        Some(total) if total > 0 => ((bytes_downloaded as f64 / total as f64) * 100.0) as i32,
        _ => 0,
    }
}

/// Calculate retry delay with exponential backoff.
/// Returns delay in milliseconds. Caps at max_delay_ms.
pub fn calculate_retry_delay(attempt: u32, base_delay_ms: u64, max_delay_ms: u64) -> u64 {
    let delay = base_delay_ms.saturating_mul(1u64 << attempt.min(10));
    delay.min(max_delay_ms)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // URL building tests
    // =========================================================================

    #[test]
    fn test_build_download_url_basic() {
        let url = build_download_url("http://example.com/q2", "maps/dm1.bsp");
        assert_eq!(url, "http://example.com/q2/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_trailing_slash() {
        let url = build_download_url("http://example.com/q2/", "maps/dm1.bsp");
        assert_eq!(url, "http://example.com/q2/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_leading_slash() {
        let url = build_download_url("http://example.com/q2", "/maps/dm1.bsp");
        assert_eq!(url, "http://example.com/q2/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_both_slashes() {
        let url = build_download_url("http://example.com/q2/", "/maps/dm1.bsp");
        assert_eq!(url, "http://example.com/q2/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_multiple_trailing_slashes() {
        let url = build_download_url("http://example.com///", "maps/dm1.bsp");
        assert_eq!(url, "http://example.com/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_empty_base() {
        let url = build_download_url("", "maps/dm1.bsp");
        assert_eq!(url, "/maps/dm1.bsp");
    }

    #[test]
    fn test_build_download_url_empty_filename() {
        let url = build_download_url("http://example.com", "");
        assert_eq!(url, "http://example.com/");
    }

    // =========================================================================
    // Path sanitization tests
    // =========================================================================

    #[test]
    fn test_sanitize_normal_path() {
        let result = sanitize_download_path("maps/dm1.bsp");
        assert_eq!(result, Some("maps/dm1.bsp".to_string()));
    }

    #[test]
    fn test_sanitize_rejects_absolute_path() {
        assert!(sanitize_download_path("/etc/passwd").is_none());
    }

    #[test]
    fn test_sanitize_rejects_backslash_absolute() {
        assert!(sanitize_download_path("\\windows\\system32\\cmd.exe").is_none());
    }

    #[test]
    fn test_sanitize_rejects_directory_traversal() {
        assert!(sanitize_download_path("../../etc/passwd").is_none());
    }

    #[test]
    fn test_sanitize_rejects_mid_path_traversal() {
        assert!(sanitize_download_path("maps/../../../etc/passwd").is_none());
    }

    #[test]
    fn test_sanitize_allows_dots_in_filename() {
        let result = sanitize_download_path("maps/dm1.bsp.bak");
        assert_eq!(result, Some("maps/dm1.bsp.bak".to_string()));
    }

    #[test]
    fn test_sanitize_allows_single_dot_component() {
        // "./maps/dm1.bsp" - single dot is fine
        let result = sanitize_download_path("./maps/dm1.bsp");
        assert!(result.is_some());
    }

    #[test]
    fn test_sanitize_normalizes_backslashes() {
        let result = sanitize_download_path("maps\\subdir\\dm1.bsp");
        assert_eq!(result, Some("maps/subdir/dm1.bsp".to_string()));
    }

    #[test]
    fn test_sanitize_rejects_drive_letter() {
        assert!(sanitize_download_path("C:\\windows\\system32").is_none());
    }

    #[test]
    fn test_sanitize_plain_filename() {
        let result = sanitize_download_path("dm1.bsp");
        assert_eq!(result, Some("dm1.bsp".to_string()));
    }

    // =========================================================================
    // Progress percentage tests
    // =========================================================================

    #[test]
    fn test_progress_percent_zero() {
        assert_eq!(calculate_progress_percent(0, Some(100)), 0);
    }

    #[test]
    fn test_progress_percent_half() {
        assert_eq!(calculate_progress_percent(50, Some(100)), 50);
    }

    #[test]
    fn test_progress_percent_full() {
        assert_eq!(calculate_progress_percent(100, Some(100)), 100);
    }

    #[test]
    fn test_progress_percent_unknown_total() {
        assert_eq!(calculate_progress_percent(500, None), 0);
    }

    #[test]
    fn test_progress_percent_zero_total() {
        assert_eq!(calculate_progress_percent(100, Some(0)), 0);
    }

    #[test]
    fn test_progress_percent_large_file() {
        // 750MB of 1GB
        let downloaded = 750_000_000u64;
        let total = 1_000_000_000u64;
        assert_eq!(calculate_progress_percent(downloaded, Some(total)), 75);
    }

    #[test]
    fn test_progress_percent_small_fraction() {
        // 1 byte of 1000
        assert_eq!(calculate_progress_percent(1, Some(1000)), 0);
    }

    #[test]
    fn test_progress_percent_overdownload() {
        // Edge case: more downloaded than total (should still compute)
        assert!(calculate_progress_percent(200, Some(100)) >= 100);
    }

    // =========================================================================
    // Retry delay calculation tests
    // =========================================================================

    #[test]
    fn test_retry_delay_first_attempt() {
        let delay = calculate_retry_delay(0, 1000, 60000);
        assert_eq!(delay, 1000);
    }

    #[test]
    fn test_retry_delay_second_attempt() {
        let delay = calculate_retry_delay(1, 1000, 60000);
        assert_eq!(delay, 2000);
    }

    #[test]
    fn test_retry_delay_third_attempt() {
        let delay = calculate_retry_delay(2, 1000, 60000);
        assert_eq!(delay, 4000);
    }

    #[test]
    fn test_retry_delay_capped_at_max() {
        let delay = calculate_retry_delay(20, 1000, 60000);
        assert_eq!(delay, 60000);
    }

    #[test]
    fn test_retry_delay_exponential_growth() {
        let d0 = calculate_retry_delay(0, 500, 1_000_000);
        let d1 = calculate_retry_delay(1, 500, 1_000_000);
        let d2 = calculate_retry_delay(2, 500, 1_000_000);
        let d3 = calculate_retry_delay(3, 500, 1_000_000);
        assert_eq!(d0, 500);
        assert_eq!(d1, 1000);
        assert_eq!(d2, 2000);
        assert_eq!(d3, 4000);
    }

    #[test]
    fn test_retry_delay_zero_base() {
        let delay = calculate_retry_delay(5, 0, 60000);
        assert_eq!(delay, 0);
    }

    // =========================================================================
    // DownloadProgress struct tests
    // =========================================================================

    #[test]
    fn test_download_progress_construction() {
        let progress = DownloadProgress {
            filename: "maps/dm1.bsp".to_string(),
            bytes_downloaded: 1024,
            total_bytes: Some(4096),
            bytes_per_second: 512,
        };
        assert_eq!(progress.filename, "maps/dm1.bsp");
        assert_eq!(progress.bytes_downloaded, 1024);
        assert_eq!(progress.total_bytes, Some(4096));
        assert_eq!(progress.bytes_per_second, 512);
    }

    #[test]
    fn test_download_progress_unknown_size() {
        let progress = DownloadProgress {
            filename: "test.bsp".to_string(),
            bytes_downloaded: 500,
            total_bytes: None,
            bytes_per_second: 100,
        };
        assert!(progress.total_bytes.is_none());
    }

    // =========================================================================
    // DownloadRequest struct tests
    // =========================================================================

    #[test]
    fn test_download_request_construction() {
        let req = DownloadRequest {
            filename: "models/player.md2".to_string(),
            dest_path: PathBuf::from("/tmp/models/player.md2"),
            id: 42,
        };
        assert_eq!(req.filename, "models/player.md2");
        assert_eq!(req.dest_path, PathBuf::from("/tmp/models/player.md2"));
        assert_eq!(req.id, 42);
    }

    // =========================================================================
    // HttpDownloadResult enum tests
    // =========================================================================

    #[test]
    fn test_http_download_result_debug() {
        // Verify all variants can be debug-printed (ensures Debug is derived)
        let _ = format!("{:?}", HttpDownloadResult::Success);
        let _ = format!("{:?}", HttpDownloadResult::NotFound);
        let _ = format!("{:?}", HttpDownloadResult::NotAvailable);
        let _ = format!("{:?}", HttpDownloadResult::Cancelled);
        let _ = format!("{:?}", HttpDownloadResult::Error("test".to_string()));
    }

    // =========================================================================
    // AsyncDownloadStatus enum tests
    // =========================================================================

    #[test]
    fn test_async_download_status_variants() {
        let progress = DownloadProgress {
            filename: "test".to_string(),
            bytes_downloaded: 0,
            total_bytes: None,
            bytes_per_second: 0,
        };
        let _ = format!("{:?}", AsyncDownloadStatus::InProgress(progress));
        let _ = format!("{:?}", AsyncDownloadStatus::Completed);
        let _ = format!("{:?}", AsyncDownloadStatus::Failed("err".to_string()));
        let _ = format!("{:?}", AsyncDownloadStatus::Cancelled);
        let _ = format!("{:?}", AsyncDownloadStatus::NotFound);
    }

    // =========================================================================
    // HTTP constants tests
    // =========================================================================

    #[test]
    fn test_http_timeout_constants() {
        assert_eq!(HTTP_CONNECT_TIMEOUT_SECS, 30);
        assert_eq!(HTTP_READ_TIMEOUT_SECS, 60);
    }

    // =========================================================================
    // Download queue management (DownloadRequest IDs)
    // =========================================================================

    #[test]
    fn test_download_request_unique_ids() {
        let r1 = DownloadRequest { filename: "a".to_string(), dest_path: PathBuf::from("a"), id: 1 };
        let r2 = DownloadRequest { filename: "b".to_string(), dest_path: PathBuf::from("b"), id: 2 };
        let r3 = DownloadRequest { filename: "c".to_string(), dest_path: PathBuf::from("c"), id: 3 };
        assert_ne!(r1.id, r2.id);
        assert_ne!(r2.id, r3.id);
        assert_ne!(r1.id, r3.id);
    }
}
