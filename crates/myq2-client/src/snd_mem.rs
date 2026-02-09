// snd_mem.rs — Sound caching and WAV loading
// Converted from: myq2-original/client/snd_mem.c

#![allow(non_snake_case, non_upper_case_globals, unused)]

use myq2_common::q_shared::*;
use myq2_common::common::com_printf;
use crate::snd_dma::{SfxCache, Sfx, WavInfo};

// ============================================================
// WAV parser state
// ============================================================

pub struct WavParser {
    data: Vec<u8>,
    pos: usize,
    iff_end: usize,
    last_chunk: usize,
    iff_data: usize,
    iff_chunk_len: i32,
}

impl WavParser {
    pub fn new(data: Vec<u8>) -> Self {
        let len = data.len();
        Self {
            data,
            pos: 0,
            iff_end: len,
            last_chunk: 0,
            iff_data: 0,
            iff_chunk_len: 0,
        }
    }

    fn get_little_short(&mut self) -> i16 {
        if self.pos + 2 > self.data.len() {
            return 0;
        }
        let val = i16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        val
    }

    fn get_little_long(&mut self) -> i32 {
        if self.pos + 4 > self.data.len() {
            return 0;
        }
        let val = i32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        val
    }

    fn find_next_chunk(&mut self, name: &[u8; 4]) -> bool {
        loop {
            self.pos = self.last_chunk;

            if self.pos >= self.iff_end {
                return false;
            }

            self.pos += 4;
            self.iff_chunk_len = self.get_little_long();
            if self.iff_chunk_len < 0 {
                return false;
            }
            self.pos -= 8;
            self.last_chunk = self.pos + 8 + ((self.iff_chunk_len as usize + 1) & !1);

            if self.pos + 4 <= self.data.len() && &self.data[self.pos..self.pos + 4] == name {
                return true;
            }
        }
    }

    fn find_chunk(&mut self, name: &[u8; 4]) -> bool {
        self.last_chunk = self.iff_data;
        self.find_next_chunk(name)
    }

    /// Parse a WAV file and return its info.
    pub fn get_wavinfo(&mut self, name: &str) -> WavInfo {
        let mut info = WavInfo::default();

        if self.data.is_empty() {
            return info;
        }

        self.iff_data = 0;
        self.iff_end = self.data.len();

        // Find "RIFF" chunk
        if !self.find_chunk(b"RIFF") {
            com_printf("Missing RIFF/WAVE chunks\n");
            return info;
        }

        if self.pos + 12 > self.data.len() || &self.data[self.pos + 8..self.pos + 12] != b"WAVE" {
            com_printf("Missing RIFF/WAVE chunks\n");
            return info;
        }

        // Get "fmt " chunk
        self.iff_data = self.pos + 12;

        if !self.find_chunk(b"fmt ") {
            com_printf("Missing fmt chunk\n");
            return info;
        }

        self.pos += 8;
        let format = self.get_little_short();
        if format != 1 {
            com_printf("Microsoft PCM format only\n");
            return info;
        }

        info.channels = self.get_little_short() as i32;
        info.rate = self.get_little_long();
        self.pos += 4 + 2; // skip avgBytesPerSec + blockAlign
        info.width = self.get_little_short() as i32 / 8;

        // Get cue chunk
        if self.find_chunk(b"cue ") {
            self.pos += 32;
            info.loopstart = self.get_little_long();

            // If the next chunk is a LIST chunk, look for a cue length marker
            if self.find_next_chunk(b"LIST")
                && self.pos + 32 <= self.data.len()
                    && &self.data[self.pos + 28..self.pos + 32] == b"mark"
                {
                    self.pos += 24;
                    let i = self.get_little_long(); // samples in loop
                    info.samples = info.loopstart + i;
                }
        } else {
            info.loopstart = -1;
        }

        // Find data chunk
        if !self.find_chunk(b"data") {
            com_printf("Missing data chunk\n");
            return info;
        }

        self.pos += 4;
        let samples = self.get_little_long() / info.width;

        if info.samples != 0 {
            if samples < info.samples {
                panic!("Sound {} has a bad loop length", name);
            }
        } else {
            info.samples = samples;
        }

        info.dataofs = self.pos as i32;

        info
    }
}

// ============================================================
// S_LoadSound
// ============================================================

/// Load sound data for the given sfx. Returns true if successful.
/// Stores WAV data at native sample rate — OpenAL handles resampling.
/// Thread-safe: only modifies the `sfx` passed in and performs file I/O.
pub fn s_load_sound<F>(sfx: &mut Sfx, mut load_file: F) -> bool
where
    F: FnMut(&str) -> Option<Vec<u8>>,
{
    if sfx.name.starts_with('*') {
        return false;
    }

    // See if still in memory
    if sfx.cache.is_some() {
        return true;
    }

    // Load it in
    let name = if let Some(ref tn) = sfx.truename {
        tn.clone()
    } else {
        sfx.name.clone()
    };

    let namebuffer = if name.starts_with('#') {
        name[1..].to_string()
    } else {
        format!("sound/{}", name)
    };

    let data = match load_file(&namebuffer) {
        Some(d) => d,
        None => {
            com_printf(&format!("Couldn't load {}\n", namebuffer));
            return false;
        }
    };

    let mut parser = WavParser::new(data.clone());
    let info = parser.get_wavinfo(&sfx.name);

    if info.channels != 1 {
        com_printf(&format!("{} is a stereo sample\n", sfx.name));
        return false;
    }

    // Store raw PCM data at native rate/width — OpenAL handles resampling.
    let dataofs = info.dataofs as usize;
    let data_len = (info.samples * info.width) as usize;
    let sound_data = if dataofs < data.len() {
        let end = (dataofs + data_len).min(data.len());
        data[dataofs..end].to_vec()
    } else {
        Vec::new()
    };

    let sc = SfxCache {
        length: info.samples,
        loopstart: info.loopstart,
        speed: info.rate,
        width: info.width,
        stereo: info.channels,
        data: sound_data,
    };

    sfx.cache = Some(Box::new(sc));

    true
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Helper to build a valid WAV file in memory
    // ============================================================

    /// Build a minimal valid PCM WAV file with given parameters.
    fn build_wav(
        channels: i16,
        sample_rate: i32,
        bits_per_sample: i16,
        num_samples: i32,
    ) -> Vec<u8> {
        let width = bits_per_sample / 8;
        let data_size = num_samples * width as i32;
        let fmt_chunk_size: i32 = 16;
        // RIFF header (12) + fmt chunk (8 + 16) + data chunk (8 + data_size)
        let riff_size = 4 + (8 + fmt_chunk_size) + (8 + data_size);

        let mut buf: Vec<u8> = Vec::new();

        // RIFF header
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&riff_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");

        // fmt chunk
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&fmt_chunk_size.to_le_bytes());
        buf.extend_from_slice(&1i16.to_le_bytes()); // format = PCM
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        let avg_bytes = sample_rate * channels as i32 * width as i32;
        buf.extend_from_slice(&avg_bytes.to_le_bytes());
        let block_align = channels * width;
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        for _ in 0..data_size {
            buf.push(0x80); // silence for 8-bit, or fill pattern
        }

        buf
    }

    /// Build a WAV with a cue chunk (loop point).
    fn build_wav_with_loop(
        channels: i16,
        sample_rate: i32,
        bits_per_sample: i16,
        num_samples: i32,
        loopstart: i32,
    ) -> Vec<u8> {
        let width = bits_per_sample / 8;
        let data_size = num_samples * width as i32;
        let fmt_chunk_size: i32 = 16;
        // cue chunk: "cue " + size(4) + 28 bytes padding + loopstart(4) = 36 bytes total
        let cue_data_size: i32 = 32; // enough to hold the cue point data
        // RIFF header (12) + fmt (24) + cue (8 + cue_data_size) + data (8 + data_size)
        let riff_size = 4 + (8 + fmt_chunk_size) + (8 + cue_data_size) + (8 + data_size);

        let mut buf: Vec<u8> = Vec::new();

        // RIFF header
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&riff_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");

        // fmt chunk
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&fmt_chunk_size.to_le_bytes());
        buf.extend_from_slice(&1i16.to_le_bytes());
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        let avg_bytes = sample_rate * channels as i32 * width as i32;
        buf.extend_from_slice(&avg_bytes.to_le_bytes());
        let block_align = channels * width;
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits_per_sample.to_le_bytes());

        // cue chunk
        buf.extend_from_slice(b"cue ");
        buf.extend_from_slice(&cue_data_size.to_le_bytes());
        // 28 bytes of padding before the loopstart value
        // (the parser does self.pos += 32 from the chunk start, which skips
        // 8 bytes of chunk header + 24 bytes of cue points, then reads the value)
        // Actually: find_chunk positions pos at chunk start, then pos += 32
        // so after the "cue " + size(4), pos is at cue data start.
        // The code does self.pos += 32 after finding the chunk, which means
        // pos = chunk_start + 32. Then it reads loopstart.
        // chunk_start points to "cue ", so chunk_start+8 = cue data
        // We need 24 bytes of padding (32-8=24) then the loopstart
        for _ in 0..24 {
            buf.push(0);
        }
        buf.extend_from_slice(&loopstart.to_le_bytes());
        // Remaining bytes to fill cue_data_size (32 - 28 = 4 already used by loopstart)
        // Total cue data = 24 pad + 4 loopstart = 28, need 32 total
        for _ in 0..4 {
            buf.push(0);
        }

        // data chunk
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        for _ in 0..data_size {
            buf.push(0x80);
        }

        buf
    }

    // ============================================================
    // WavParser construction tests
    // ============================================================

    #[test]
    fn test_wav_parser_new() {
        let data = vec![1, 2, 3, 4, 5];
        let parser = WavParser::new(data.clone());
        assert_eq!(parser.data, data);
        assert_eq!(parser.pos, 0);
        assert_eq!(parser.iff_end, 5);
        assert_eq!(parser.last_chunk, 0);
        assert_eq!(parser.iff_data, 0);
        assert_eq!(parser.iff_chunk_len, 0);
    }

    #[test]
    fn test_wav_parser_empty() {
        let parser = WavParser::new(Vec::new());
        assert_eq!(parser.iff_end, 0);
        assert_eq!(parser.pos, 0);
    }

    // ============================================================
    // get_little_short / get_little_long tests
    // ============================================================

    #[test]
    fn test_get_little_short() {
        let mut parser = WavParser::new(vec![0x34, 0x12]);
        let val = parser.get_little_short();
        assert_eq!(val, 0x1234);
        assert_eq!(parser.pos, 2);
    }

    #[test]
    fn test_get_little_short_boundary() {
        // Not enough data
        let mut parser = WavParser::new(vec![0x34]);
        let val = parser.get_little_short();
        assert_eq!(val, 0);
        assert_eq!(parser.pos, 0); // pos unchanged
    }

    #[test]
    fn test_get_little_short_negative() {
        let mut parser = WavParser::new(vec![0xFF, 0xFF]);
        let val = parser.get_little_short();
        assert_eq!(val, -1);
    }

    #[test]
    fn test_get_little_long() {
        let mut parser = WavParser::new(vec![0x78, 0x56, 0x34, 0x12]);
        let val = parser.get_little_long();
        assert_eq!(val, 0x12345678);
        assert_eq!(parser.pos, 4);
    }

    #[test]
    fn test_get_little_long_boundary() {
        let mut parser = WavParser::new(vec![0x78, 0x56, 0x34]);
        let val = parser.get_little_long();
        assert_eq!(val, 0);
        assert_eq!(parser.pos, 0);
    }

    #[test]
    fn test_get_little_long_negative() {
        let mut parser = WavParser::new(vec![0xFF, 0xFF, 0xFF, 0xFF]);
        let val = parser.get_little_long();
        assert_eq!(val, -1);
    }

    #[test]
    fn test_get_little_short_sequential() {
        let mut parser = WavParser::new(vec![0x01, 0x00, 0x02, 0x00, 0x03, 0x00]);
        assert_eq!(parser.get_little_short(), 1);
        assert_eq!(parser.get_little_short(), 2);
        assert_eq!(parser.get_little_short(), 3);
        assert_eq!(parser.pos, 6);
    }

    // ============================================================
    // WAV file parsing (get_wavinfo) tests
    // ============================================================

    #[test]
    fn test_get_wavinfo_empty() {
        let mut parser = WavParser::new(Vec::new());
        let info = parser.get_wavinfo("test.wav");
        assert_eq!(info.rate, 0);
        assert_eq!(info.width, 0);
        assert_eq!(info.channels, 0);
        assert_eq!(info.samples, 0);
    }

    #[test]
    fn test_get_wavinfo_valid_mono_8bit_11025() {
        let wav = build_wav(1, 11025, 8, 11025);
        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");

        assert_eq!(info.channels, 1);
        assert_eq!(info.rate, 11025);
        assert_eq!(info.width, 1); // 8-bit / 8 = 1 byte
        assert_eq!(info.samples, 11025);
        assert_eq!(info.loopstart, -1); // no cue chunk
        assert!(info.dataofs > 0);
    }

    #[test]
    fn test_get_wavinfo_valid_mono_16bit_22050() {
        let wav = build_wav(1, 22050, 16, 22050);
        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");

        assert_eq!(info.channels, 1);
        assert_eq!(info.rate, 22050);
        assert_eq!(info.width, 2); // 16-bit / 8 = 2 bytes
        assert_eq!(info.samples, 22050);
        assert_eq!(info.loopstart, -1);
    }

    #[test]
    fn test_get_wavinfo_valid_mono_16bit_44100() {
        let wav = build_wav(1, 44100, 16, 44100);
        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");

        assert_eq!(info.channels, 1);
        assert_eq!(info.rate, 44100);
        assert_eq!(info.width, 2);
        assert_eq!(info.samples, 44100);
    }

    #[test]
    fn test_get_wavinfo_stereo() {
        let wav = build_wav(2, 22050, 16, 22050);
        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");

        assert_eq!(info.channels, 2);
        assert_eq!(info.rate, 22050);
        assert_eq!(info.width, 2);
    }

    #[test]
    fn test_get_wavinfo_with_loop_point() {
        let wav = build_wav_with_loop(1, 22050, 16, 22050, 5000);
        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");

        assert_eq!(info.channels, 1);
        assert_eq!(info.rate, 22050);
        assert_eq!(info.width, 2);
        assert_eq!(info.loopstart, 5000);
    }

    #[test]
    fn test_get_wavinfo_no_riff() {
        // Invalid WAV: no RIFF header
        let data = vec![0u8; 100];
        let mut parser = WavParser::new(data);
        let info = parser.get_wavinfo("test.wav");
        assert_eq!(info.rate, 0);
        assert_eq!(info.channels, 0);
    }

    #[test]
    fn test_get_wavinfo_riff_but_no_wave() {
        // Has RIFF header but no WAVE marker
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&100i32.to_le_bytes());
        data.extend_from_slice(b"XXXX"); // not WAVE
        data.extend_from_slice(&vec![0u8; 96]);

        let mut parser = WavParser::new(data);
        let info = parser.get_wavinfo("test.wav");
        assert_eq!(info.rate, 0);
    }

    // ============================================================
    // Data offset validation
    // ============================================================

    #[test]
    fn test_get_wavinfo_data_offset_is_after_headers() {
        let wav = build_wav(1, 11025, 8, 100);
        let mut parser = WavParser::new(wav.clone());
        let info = parser.get_wavinfo("test.wav");

        // Data offset should point past all headers
        assert!(info.dataofs > 0);
        let dataofs = info.dataofs as usize;
        assert!(dataofs <= wav.len());
        // The remaining data from dataofs should be the sample data
        let remaining = wav.len() - dataofs;
        assert_eq!(remaining as i32, info.samples * info.width);
    }

    // ============================================================
    // s_load_sound tests
    // ============================================================

    #[test]
    fn test_s_load_sound_skips_star_prefix() {
        let mut sfx = Sfx {
            name: "*splash".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| None);
        assert!(!result);
        assert!(sfx.cache.is_none());
    }

    #[test]
    fn test_s_load_sound_returns_true_if_cached() {
        let mut sfx = Sfx {
            name: "test.wav".to_string(),
            registration_sequence: 0,
            cache: Some(Box::new(SfxCache::default())),
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| panic!("should not load"));
        assert!(result);
    }

    #[test]
    fn test_s_load_sound_file_not_found() {
        let mut sfx = Sfx {
            name: "test.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| None);
        assert!(!result);
        assert!(sfx.cache.is_none());
    }

    #[test]
    fn test_s_load_sound_path_construction() {
        // Normal name: "sound/" + name
        let mut sfx = Sfx {
            name: "weapons/blaster.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let mut requested_path = String::new();
        let _ = s_load_sound(&mut sfx, |path| {
            requested_path = path.to_string();
            None
        });
        assert_eq!(requested_path, "sound/weapons/blaster.wav");
    }

    #[test]
    fn test_s_load_sound_hash_prefix_path() {
        // Name starting with '#' strips it and uses the name directly
        let mut sfx = Sfx {
            name: "#music/track01.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let mut requested_path = String::new();
        let _ = s_load_sound(&mut sfx, |path| {
            requested_path = path.to_string();
            None
        });
        assert_eq!(requested_path, "music/track01.wav");
    }

    #[test]
    fn test_s_load_sound_truename_overrides() {
        let mut sfx = Sfx {
            name: "alias.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: Some("real_sound.wav".to_string()),
        };

        let mut requested_path = String::new();
        let _ = s_load_sound(&mut sfx, |path| {
            requested_path = path.to_string();
            None
        });
        assert_eq!(requested_path, "sound/real_sound.wav");
    }

    #[test]
    fn test_s_load_sound_valid_wav() {
        let wav = build_wav(1, 22050, 16, 1000);

        let mut sfx = Sfx {
            name: "test.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| Some(wav.clone()));
        assert!(result);

        let cache = sfx.cache.as_ref().unwrap();
        assert_eq!(cache.length, 1000);
        assert_eq!(cache.speed, 22050);
        assert_eq!(cache.width, 2);
        assert_eq!(cache.stereo, 1);
        assert_eq!(cache.loopstart, -1);
        // Data should contain 1000 samples * 2 bytes = 2000 bytes
        assert_eq!(cache.data.len(), 2000);
    }

    #[test]
    fn test_s_load_sound_rejects_stereo() {
        let wav = build_wav(2, 22050, 16, 1000);

        let mut sfx = Sfx {
            name: "stereo.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| Some(wav.clone()));
        assert!(!result);
        assert!(sfx.cache.is_none());
    }

    #[test]
    fn test_s_load_sound_8bit_mono() {
        let wav = build_wav(1, 11025, 8, 500);

        let mut sfx = Sfx {
            name: "test8.wav".to_string(),
            registration_sequence: 0,
            cache: None,
            truename: None,
        };

        let result = s_load_sound(&mut sfx, |_| Some(wav.clone()));
        assert!(result);

        let cache = sfx.cache.as_ref().unwrap();
        assert_eq!(cache.length, 500);
        assert_eq!(cache.speed, 11025);
        assert_eq!(cache.width, 1);
        assert_eq!(cache.data.len(), 500);
    }

    // ============================================================
    // WavInfo default test
    // ============================================================

    #[test]
    fn test_wavinfo_default() {
        let info = WavInfo::default();
        assert_eq!(info.rate, 0);
        assert_eq!(info.width, 0);
        assert_eq!(info.channels, 0);
        assert_eq!(info.loopstart, 0);
        assert_eq!(info.samples, 0);
        assert_eq!(info.dataofs, 0);
    }

    // ============================================================
    // SfxCache default test
    // ============================================================

    #[test]
    fn test_sfxcache_default() {
        let cache = SfxCache::default();
        assert_eq!(cache.length, 0);
        assert_eq!(cache.loopstart, -1);
        assert_eq!(cache.speed, 0);
        assert_eq!(cache.width, 0);
        assert_eq!(cache.stereo, 0);
        assert!(cache.data.is_empty());
    }

    // ============================================================
    // Sample calculation tests
    // ============================================================

    #[test]
    fn test_samples_from_data_size_8bit() {
        // For 8-bit audio: samples = data_size / width = data_size / 1
        let data_size = 11025i32;
        let width = 1i32;
        let samples = data_size / width;
        assert_eq!(samples, 11025);
    }

    #[test]
    fn test_samples_from_data_size_16bit() {
        // For 16-bit audio: samples = data_size / width = data_size / 2
        let data_size = 44100i32;
        let width = 2i32;
        let samples = data_size / width;
        assert_eq!(samples, 22050);
    }

    // ============================================================
    // find_chunk logic test
    // ============================================================

    #[test]
    fn test_find_chunk_in_valid_wav() {
        let wav = build_wav(1, 11025, 8, 100);
        let mut parser = WavParser::new(wav);

        // Reset IFF data to search from start
        parser.iff_data = 0;
        parser.iff_end = parser.data.len();

        assert!(parser.find_chunk(b"RIFF"));
    }

    #[test]
    fn test_find_chunk_missing_chunk() {
        let wav = build_wav(1, 11025, 8, 100);
        let mut parser = WavParser::new(wav);
        parser.iff_data = 0;
        parser.iff_end = parser.data.len();

        // "XYZ " chunk does not exist
        assert!(!parser.find_chunk(b"XYZ "));
    }

    // ============================================================
    // Non-PCM format rejection
    // ============================================================

    #[test]
    fn test_get_wavinfo_non_pcm_format() {
        // Build a WAV with a non-PCM format tag (e.g., 3 = IEEE float)
        let mut wav = build_wav(1, 22050, 16, 100);
        // Find the format field and change it
        // After "fmt " + size(4), the format is at offset: find "fmt " + 8
        if let Some(pos) = wav.windows(4).position(|w| w == b"fmt ") {
            let fmt_pos = pos + 8; // skip "fmt " + chunk_size
            wav[fmt_pos] = 3; // IEEE float format
            wav[fmt_pos + 1] = 0;
        }

        let mut parser = WavParser::new(wav);
        let info = parser.get_wavinfo("test.wav");
        // Should fail: only PCM format (1) is accepted
        assert_eq!(info.rate, 0);
    }

    // ============================================================
    // Byte order handling tests
    // ============================================================

    #[test]
    fn test_little_endian_consistency() {
        // Verify that our WAV builder produces correct LE values
        let wav = build_wav(1, 44100, 16, 1000);

        // Find the sample rate in the fmt chunk
        if let Some(pos) = wav.windows(4).position(|w| w == b"fmt ") {
            let rate_pos = pos + 12; // "fmt "(4) + size(4) + format(2) + channels(2) = offset 12
            let rate = i32::from_le_bytes([
                wav[rate_pos],
                wav[rate_pos + 1],
                wav[rate_pos + 2],
                wav[rate_pos + 3],
            ]);
            assert_eq!(rate, 44100);
        } else {
            panic!("fmt chunk not found");
        }
    }
}
