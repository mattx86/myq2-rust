// compression.rs -- Zlib packet compression for R1Q2/Q2Pro protocols
//
// Protocol 35+ uses zlib compression for large packets. The compression uses
// raw deflate (no zlib header) with windowBits = -15 for compatibility.

use flate2::read::{DeflateDecoder, DeflateEncoder};
use flate2::Compression;
use std::io::Read;

/// Minimum packet size to consider for compression.
/// Packets smaller than this are not worth compressing.
pub const MIN_COMPRESS_SIZE: usize = 100;

/// Compression threshold - only compress if we save at least this percentage.
pub const COMPRESS_THRESHOLD_PERCENT: usize = 20;

/// Maximum decompressed packet size to prevent decompression bombs.
pub const MAX_DECOMPRESS_SIZE: usize = 65536;

/// Compress data using raw deflate (no zlib header).
///
/// Returns `Some(compressed_data)` if compression is beneficial (saves >20% space),
/// otherwise returns `None` to indicate the original data should be sent.
///
/// # Arguments
/// * `data` - The data to compress
///
/// # Returns
/// * `Some(Vec<u8>)` - Compressed data if compression saved space
/// * `None` - If compression didn't help or data is too small
pub fn compress_packet(data: &[u8]) -> Option<Vec<u8>> {
    // Don't bother compressing small packets
    if data.len() < MIN_COMPRESS_SIZE {
        return None;
    }

    // Use default compression level (6) for good balance of speed/ratio
    let mut encoder = DeflateEncoder::new(data, Compression::default());
    let mut compressed = Vec::with_capacity(data.len());

    if encoder.read_to_end(&mut compressed).is_err() {
        return None;
    }

    // Only use compression if we save at least COMPRESS_THRESHOLD_PERCENT
    let threshold = data.len() * (100 - COMPRESS_THRESHOLD_PERCENT) / 100;
    if compressed.len() < threshold {
        Some(compressed)
    } else {
        None
    }
}

/// Compress data unconditionally using raw deflate.
///
/// Unlike `compress_packet`, this always returns compressed data regardless
/// of whether it's smaller than the original. Useful for downloads where
/// the compressed flag is always set.
///
/// # Arguments
/// * `data` - The data to compress
///
/// # Returns
/// * `Ok(Vec<u8>)` - Compressed data
/// * `Err(String)` - Error message if compression failed
pub fn compress_data(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(data, Compression::default());
    let mut compressed = Vec::with_capacity(data.len());

    encoder
        .read_to_end(&mut compressed)
        .map_err(|e| format!("Compression failed: {}", e))?;

    Ok(compressed)
}

/// Decompress raw deflate data.
///
/// # Arguments
/// * `data` - The compressed data
/// * `max_size` - Maximum allowed decompressed size (to prevent decompression bombs)
///
/// # Returns
/// * `Some(Vec<u8>)` - Decompressed data
/// * `None` - If decompression failed or result exceeds max_size
pub fn decompress_packet(data: &[u8], max_size: usize) -> Option<Vec<u8>> {
    let max_size = max_size.min(MAX_DECOMPRESS_SIZE);

    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::with_capacity(max_size.min(data.len() * 4));

    // Read in chunks to check size limit
    let mut buffer = [0u8; 4096];
    loop {
        match decoder.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => {
                if decompressed.len() + n > max_size {
                    // Decompressed size exceeds limit
                    return None;
                }
                decompressed.extend_from_slice(&buffer[..n]);
            }
            Err(_) => return None,
        }
    }

    Some(decompressed)
}

/// Decompress data with known uncompressed size.
///
/// This is used for `svc_zdownload` where the uncompressed size is known.
///
/// # Arguments
/// * `data` - The compressed data
/// * `uncompressed_size` - Expected size of decompressed data
///
/// # Returns
/// * `Ok(Vec<u8>)` - Decompressed data
/// * `Err(String)` - Error message if decompression failed or size mismatch
pub fn decompress_with_size(data: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, String> {
    if uncompressed_size > MAX_DECOMPRESS_SIZE {
        return Err(format!(
            "Uncompressed size {} exceeds maximum {}",
            uncompressed_size, MAX_DECOMPRESS_SIZE
        ));
    }

    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::with_capacity(uncompressed_size);

    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Decompression failed: {}", e))?;

    if decompressed.len() != uncompressed_size {
        return Err(format!(
            "Size mismatch: expected {}, got {}",
            uncompressed_size,
            decompressed.len()
        ));
    }

    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"Hello, World! This is a test of the compression system. \
            We need enough data to make compression worthwhile, so let's add some \
            repetitive content. AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

        if let Some(compressed) = compress_packet(original) {
            let decompressed = decompress_packet(&compressed, original.len() * 2).unwrap();
            assert_eq!(original.as_slice(), decompressed.as_slice());
        }
    }

    #[test]
    fn test_small_data_not_compressed() {
        let small = b"tiny";
        assert!(compress_packet(small).is_none());
    }

    #[test]
    fn test_incompressible_data() {
        // Random-looking data doesn't compress well
        let random: Vec<u8> = (0..200).map(|i| ((i * 17 + 31) % 256) as u8).collect();
        // This may or may not compress depending on the pattern
        // The test just ensures it doesn't panic
        let _ = compress_packet(&random);
    }

    #[test]
    fn test_decompress_size_limit() {
        let original = vec![0u8; 10000];
        let compressed = compress_data(&original).unwrap();

        // Should fail with small limit
        assert!(decompress_packet(&compressed, 100).is_none());

        // Should succeed with adequate limit
        assert!(decompress_packet(&compressed, 20000).is_some());
    }
}
