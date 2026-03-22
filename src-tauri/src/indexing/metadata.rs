//! Audio metadata extraction using symphonia and lofty.
//!
//! This module provides pure Rust audio metadata extraction without
//! requiring external tools like FFmpeg.

use lofty::prelude::*;
use rayon::prelude::*;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Audio metadata extracted from a file.
#[derive(Debug, Clone, Default)]
pub struct AudioMetadata {
    pub duration_ms: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_rate: Option<i64>,
    pub codec: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub comment: Option<String>,
}

/// Result of extracting metadata from a file.
#[derive(Debug)]
pub struct MetadataResult {
    pub path: String,
    pub metadata: Option<AudioMetadata>,
    pub error: Option<String>,
}

/// Extract audio metadata from a file.
///
/// Uses lofty first (single file open for duration + tags).
/// Falls back to symphonia only if lofty can't get duration.
pub fn extract_metadata(path: &Path) -> Result<AudioMetadata, String> {
    let mut metadata = AudioMetadata::default();

    // Try lofty first - gets duration AND tags in one file open
    let lofty_ok = extract_with_lofty(path, &mut metadata).is_ok();

    // Only use symphonia if lofty couldn't get duration
    if metadata.duration_ms.is_none() {
        let _ = extract_with_symphonia(path, &mut metadata);
    }

    // At minimum we should have duration
    if metadata.duration_ms.is_none() && !lofty_ok {
        return Err("Could not extract metadata".to_string());
    }

    Ok(metadata)
}

/// Extract audio properties (duration, sample rate, etc.) using symphonia.
fn extract_with_symphonia(path: &Path, metadata: &mut AudioMetadata) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions {
        enable_gapless: false,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| e.to_string())?;

    let format = probed.format;

    // Get the default track (usually the first audio track)
    if let Some(track) = format.default_track() {
        let codec_params = &track.codec_params;

        // Sample rate
        if let Some(sample_rate) = codec_params.sample_rate {
            metadata.sample_rate = Some(sample_rate as i64);
        }

        // Channels
        if let Some(channels) = codec_params.channels {
            metadata.channels = Some(channels.count() as i64);
        }

        // Codec name
        metadata.codec = Some(get_codec_name(codec_params.codec));

        // Calculate duration
        if let (Some(n_frames), Some(sample_rate)) =
            (codec_params.n_frames, codec_params.sample_rate)
        {
            if sample_rate > 0 {
                let duration_secs = n_frames as f64 / sample_rate as f64;
                metadata.duration_ms = Some((duration_secs * 1000.0) as i64);
            }
        } else if let Some(time_base) = codec_params.time_base {
            // Try using time_base for duration
            if let Some(n_frames) = codec_params.n_frames {
                let duration = time_base.calc_time(n_frames);
                metadata.duration_ms = Some((duration.seconds * 1000) as i64);
            }
        }

        // Bit rate (bits per second)
        if let Some(bits_per_sample) = codec_params.bits_per_sample {
            if let Some(sample_rate) = codec_params.sample_rate {
                if let Some(channels) = codec_params.channels {
                    let bit_rate = bits_per_sample as i64
                        * sample_rate as i64
                        * channels.count() as i64;
                    metadata.bit_rate = Some(bit_rate);
                }
            }
        }
    }

    Ok(())
}

/// Extract all metadata using lofty (duration, audio properties, and tags in one pass).
fn extract_with_lofty(path: &Path, metadata: &mut AudioMetadata) -> Result<(), String> {
    let tagged_file = lofty::read_from_path(path).map_err(|e| e.to_string())?;

    // Get audio properties (duration, sample rate, channels, bitrate)
    let properties = tagged_file.properties();

    let duration = properties.duration();
    if !duration.is_zero() {
        metadata.duration_ms = Some(duration.as_millis() as i64);
    }

    if let Some(sample_rate) = properties.sample_rate() {
        metadata.sample_rate = Some(sample_rate as i64);
    }

    if let Some(channels) = properties.channels() {
        metadata.channels = Some(channels as i64);
    }

    if let Some(bitrate) = properties.audio_bitrate() {
        metadata.bit_rate = Some((bitrate * 1000) as i64); // Convert kbps to bps
    }

    // Get tags (title, artist, etc.)
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
    if let Some(tag) = tag {
        metadata.title = tag.title().map(|s| s.to_string());
        metadata.artist = tag.artist().map(|s| s.to_string());
        metadata.album = tag.album().map(|s| s.to_string());
        metadata.genre = tag.genre().map(|s| s.to_string());
        metadata.comment = tag.comment().map(|s| s.to_string());
    }

    // Detect codec from file type
    if metadata.codec.is_none() {
        metadata.codec = Some(match tagged_file.file_type() {
            lofty::file::FileType::Aac => "aac",
            lofty::file::FileType::Aiff => "aiff",
            lofty::file::FileType::Flac => "flac",
            lofty::file::FileType::Mpeg => "mp3",
            lofty::file::FileType::Opus => "opus",
            lofty::file::FileType::Vorbis => "vorbis",
            lofty::file::FileType::Wav => "wav",
            lofty::file::FileType::Mp4 => "aac",
            _ => "unknown",
        }.to_string());
    }

    Ok(())
}

/// Get a human-readable codec name from symphonia's codec type.
fn get_codec_name(codec: symphonia::core::codecs::CodecType) -> String {
    // Use the codec's short name or fall back to a generic description
    let type_str = format!("{:?}", codec);

    // Extract a clean name from the debug representation
    if type_str.contains("Mp3") || type_str.contains("MP3") {
        "mp3".to_string()
    } else if type_str.contains("Aac") || type_str.contains("AAC") {
        "aac".to_string()
    } else if type_str.contains("Flac") || type_str.contains("FLAC") {
        "flac".to_string()
    } else if type_str.contains("Vorbis") || type_str.contains("VORBIS") {
        "vorbis".to_string()
    } else if type_str.contains("Opus") || type_str.contains("OPUS") {
        "opus".to_string()
    } else if type_str.contains("Pcm") || type_str.contains("PCM") {
        "pcm".to_string()
    } else if type_str.contains("Alac") || type_str.contains("ALAC") {
        "alac".to_string()
    } else if type_str.contains("Adpcm") || type_str.contains("ADPCM") {
        "adpcm".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Extract metadata from multiple files in parallel.
///
/// Uses rayon for parallel processing with progress reporting.
/// Progress is reported every PROGRESS_INTERVAL files to avoid overwhelming the event system.
pub fn extract_batch<F>(
    paths: &[String],
    progress_callback: Option<F>,
) -> Vec<MetadataResult>
where
    F: Fn(usize, usize, &str) + Sync + Send,
{
    if paths.is_empty() {
        return Vec::new();
    }

    let total = paths.len();
    let processed = Arc::new(AtomicUsize::new(0));
    // Report progress every 10 files to avoid event flooding
    const PROGRESS_INTERVAL: usize = 10;

    paths
        .par_iter()
        .map(|path| {
            let result = match extract_metadata(Path::new(path)) {
                Ok(meta) => MetadataResult {
                    path: path.clone(),
                    metadata: Some(meta),
                    error: None,
                },
                Err(e) => MetadataResult {
                    path: path.clone(),
                    metadata: None,
                    error: Some(e),
                },
            };

            // Report progress periodically
            let current = processed.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(ref callback) = progress_callback {
                // Report every PROGRESS_INTERVAL files or at completion
                if current % PROGRESS_INTERVAL == 0 || current == total {
                    callback(current, total, path);
                }
            }

            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_codec_name() {
        // Basic test that the function doesn't panic
        let codec = symphonia::core::codecs::CODEC_TYPE_NULL;
        let name = get_codec_name(codec);
        assert!(!name.is_empty());
    }
}
