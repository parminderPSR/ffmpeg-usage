//! Shared FFmpeg checks and supported-audio file discovery (non-recursive).

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extensions treated as inputs for batch tools (`m4a`, `mp3`, `flac`, `wav`).
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["m4a", "mp3", "flac", "wav"];

/// Whether `ext` (no dot) is a supported audio extension, ASCII case-insensitive.
pub fn is_supported_audio_extension(ext: &str) -> bool {
    SUPPORTED_AUDIO_EXTENSIONS
        .iter()
        .any(|&x| x.eq_ignore_ascii_case(ext))
}

/// Exit with a clear message if `ffmpeg` is not on `PATH`.
pub fn require_ffmpeg() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!(
            "Error: ffmpeg binary not found. Please install ffmpeg and ensure it is in your PATH."
        );
        std::process::exit(1);
    }
}

/// Non-recursive list of supported audio files in `dir`, sorted by path.
pub fn list_supported_audio_files_in_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if is_supported_audio_extension(ext) {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}
