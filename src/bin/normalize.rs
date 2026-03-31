//! `normalize` — FFmpeg two-pass (or single-pass) loudness normalization CLI.
//!
//! Loudness normalization via FFmpeg `loudnorm` (single- or two-pass). Any arguments after `--` are
//! forwarded to `ffmpeg` (inserted after `-af`, before the output file).

use clap::Parser;

#[path = "../ffmpeg_tools.rs"]
mod ffmpeg_tools;

use ffmpeg_tools::{list_supported_audio_files_in_dir, require_ffmpeg};
use regex::Regex;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Target loudness for [`apply_loudnorm`] (`I`, `TP`, `LRA` match FFmpeg `loudnorm` options).
#[derive(Clone, Debug, PartialEq)]
struct LoudnormConfig {
    /// Integrated loudness target (LUFS), e.g. -16 for EBU R128 broadcast.
    integrated_lufs: f64,
    /// Maximum true peak (dBFS), e.g. -1.5.
    true_peak_dbfs: f64,
    /// Loudness range (LU), e.g. 11.
    loudness_range: f64,
}

impl Default for LoudnormConfig {
    fn default() -> Self {
        Self {
            integrated_lufs: -16.0,
            true_peak_dbfs: -1.5,
            loudness_range: 11.0,
        }
    }
}

impl LoudnormConfig {
    /// Streaming-oriented defaults (approx. -14 LUFS integrated).
    #[cfg(test)]
    fn streaming() -> Self {
        Self {
            integrated_lufs: -14.0,
            true_peak_dbfs: -1.0,
            loudness_range: 11.0,
        }
    }

    /// FFmpeg `-af` value for this config (single-pass `loudnorm`).
    fn loudnorm_filter(&self) -> String {
        format!(
            "loudnorm=I={}:TP={}:LRA={}",
            self.integrated_lufs, self.true_peak_dbfs, self.loudness_range
        )
    }
}

/// Run FFmpeg single-pass `loudnorm` on `input` and write `output`.
///
/// `ffmpeg_before_output` are extra FFmpeg arguments inserted after `-af <filter>` and before the
/// output filename (e.g. `-ar 48000`, `-c:a libmp3lame`, `-b:a 192k`). Pass `&[]` if none.
fn apply_loudnorm(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    config: &LoudnormConfig,
    ffmpeg_before_output: &[OsString],
) -> io::Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(input.as_ref())
        .arg("-af")
        .arg(config.loudnorm_filter());
    for a in ffmpeg_before_output {
        cmd.arg(a);
    }
    cmd.arg(output.as_ref());
    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("ffmpeg loudnorm failed: {}", stderr.trim()),
        ));
    }
    Ok(())
}

/// Parse a field from `loudnorm` JSON printed to stderr (pass 1).
fn extract_loudnorm_json_field(stderr: &str, key: &str) -> Option<String> {
    let quoted = Regex::new(&format!(r#""{}"\s*:\s*"([^"]*)""#, key)).ok()?;
    if let Some(c) = quoted.captures(stderr) {
        return c.get(1).map(|m| m.as_str().to_string());
    }
    let num = Regex::new(&format!(r#""{}"\s*:\s*(-?[\d.]+)"#, key)).ok()?;
    num.captures(stderr)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

/// Two-pass `loudnorm` (analyze, then encode with measured values). Better quality than [`apply_loudnorm`].
///
/// `ffmpeg_before_output` is applied on pass 2 only (after `-af`, before `-c:v`/`-ar` defaults and the
/// output path). Pass 1 (analysis) does not receive these flags. Use `&[]` if none.
fn loudnorm_two_pass(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    integrated_lufs: f64,
    true_peak_dbfs: f64,
    loudness_range: f64,
    ffmpeg_before_output: &[OsString],
) -> io::Result<()> {
    let input = input.as_ref();
    let output = output.as_ref();
    let af_pass1 = format!(
        "loudnorm=I={}:TP={}:LRA={}:print_format=json",
        integrated_lufs, true_peak_dbfs, loudness_range
    );
    let analysis = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(input)
        .arg("-af")
        .arg(&af_pass1)
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()?;
    let stderr = String::from_utf8_lossy(&analysis.stderr);
    let measured_i = extract_loudnorm_json_field(&stderr, "input_i").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "loudnorm analysis did not return input_i; check ffmpeg and input file",
        )
    })?;
    let measured_tp = extract_loudnorm_json_field(&stderr, "input_tp").unwrap_or_default();
    let measured_lra = extract_loudnorm_json_field(&stderr, "input_lra").unwrap_or_default();
    let measured_thresh = extract_loudnorm_json_field(&stderr, "input_thresh").unwrap_or_default();
    let offset = extract_loudnorm_json_field(&stderr, "target_offset").unwrap_or_default();

    let af_pass2 = format!(
        "loudnorm=I={}:TP={}:LRA={}:measured_i={}:measured_tp={}:measured_lra={}:measured_thresh={}:offset={}:linear=true",
        integrated_lufs,
        true_peak_dbfs,
        loudness_range,
        measured_i,
        measured_tp,
        measured_lra,
        measured_thresh,
        offset
    );
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner")
        .arg("-stats")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-af")
        .arg(&af_pass2);
    for a in ffmpeg_before_output {
        cmd.arg(a);
    }
    cmd.arg("-c:v").arg("copy").arg("-ar").arg("48000").arg(output);
    let status = cmd.status()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "ffmpeg loudnorm pass 2 failed",
        ));
    }
    Ok(())
}

/// Default output path: `<parent>/<stem>_normalized.<ext>` (same base name as input; falls back to `.wav` if no extension).
fn default_normalized_path(input: impl AsRef<Path>) -> PathBuf {
    let input = input.as_ref();
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("wav");
    input
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}_normalized.{}", stem, ext))
}

#[derive(Parser, Debug)]
#[command(
    name = "normalize",
    version,
    about = "Loudness normalization (FFmpeg loudnorm)",
    after_help = "Extra FFmpeg flags go after -- (applied before the output path), e.g.:\n  \
                   normalize -i in.wav -o out.mp3 --single-pass -- -c:a libmp3lame -b:a 192k\n\
                   When -i is a directory, -o is not allowed; each file becomes <stem>_normalized.<ext> beside it."
)]
struct Args {
    /// Input audio file, or a directory containing m4a/mp3/flac/wav (non-recursive)
    #[arg(short = 'i', long, required = true)]
    input: String,

    /// Output file when -i is a single file (default: <stem>_normalized.<ext>). Not allowed when -i is a directory.
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Target integrated loudness (LUFS)
    #[arg(short = 'l', long, default_value_t = -16.0, allow_hyphen_values = true)]
    lufs: f64,

    /// Target true peak (dBFS)
    #[arg(short = 't', long, default_value_t = -1.5, allow_hyphen_values = true)]
    tp: f64,

    /// Loudness range (LU)
    #[arg(long, default_value_t = 11.0)]
    lra: f64,

    /// Single-pass loudnorm (faster; two-pass is default, better quality)
    #[arg(long)]
    single_pass: bool,
}

/// Split `argv` at the first standalone `--`: left side is parsed by clap, right side is passed to ffmpeg.
fn split_at_double_dash(mut args: Vec<OsString>) -> (Vec<OsString>, Vec<OsString>) {
    if let Some(i) = args
        .iter()
        .position(|s| s.as_os_str() == OsStr::new("--"))
    {
        let extra = args.split_off(i + 1);
        let _ = args.pop(); // remove "--"
        (args, extra)
    } else {
        (args, vec![])
    }
}

fn normalize_one(
    in_path: &Path,
    out_path: &Path,
    args: &Args,
    ffmpeg_extra: &[OsString],
) -> io::Result<()> {
    if args.single_pass {
        let cfg = LoudnormConfig {
            integrated_lufs: args.lufs,
            true_peak_dbfs: args.tp,
            loudness_range: args.lra,
        };
        eprintln!("normalize: single-pass → {}", out_path.display());
        if !ffmpeg_extra.is_empty() {
            eprintln!("normalize: ffmpeg extras: {:?}", ffmpeg_extra);
        }
        apply_loudnorm(in_path, out_path, &cfg, ffmpeg_extra)?;
    } else {
        eprintln!("normalize: two-pass → {}", out_path.display());
        if !ffmpeg_extra.is_empty() {
            eprintln!("normalize: ffmpeg extras (pass 2): {:?}", ffmpeg_extra);
        }
        loudnorm_two_pass(
            in_path,
            out_path,
            args.lufs,
            args.tp,
            args.lra,
            ffmpeg_extra,
        )?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let argv: Vec<OsString> = std::env::args_os().collect();
    let (for_clap, ffmpeg_extra) = split_at_double_dash(argv);
    let args = Args::try_parse_from(for_clap).unwrap_or_else(|e| e.exit());
    require_ffmpeg();

    let input = Path::new(&args.input);
    if !input.exists() {
        eprintln!("normalize: input not found: {}", args.input);
        std::process::exit(1);
    }

    if input.is_dir() {
        if args.output.is_some() {
            eprintln!(
                "normalize: -o/--output cannot be used when -i is a directory (each file is written as <stem>_normalized.<ext> beside the source)"
            );
            std::process::exit(1);
        }
        let files = list_supported_audio_files_in_dir(input)?;
        if files.is_empty() {
            eprintln!(
                "normalize: no supported audio files (m4a, mp3, flac, wav) in {}",
                input.display()
            );
            std::process::exit(1);
        }
        eprintln!(
            "normalize: batch {} file(s) in {}",
            files.len(),
            input.display()
        );
        for file in &files {
            let out_path = default_normalized_path(file);
            eprintln!("normalize: {} → {}", file.display(), out_path.display());
            normalize_one(file, &out_path, &args, &ffmpeg_extra)?;
        }
    } else if input.is_file() {
        let out_path = args
            .output
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_normalized_path(input));
        normalize_one(input, &out_path, &args, &ffmpeg_extra)?;
    } else {
        eprintln!("normalize: input must be a file or directory");
        std::process::exit(1);
    }

    eprintln!("normalize: done");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loudnorm_config_default_and_streaming() {
        let d = LoudnormConfig::default();
        assert_eq!(d.integrated_lufs, -16.0);
        assert_eq!(d.true_peak_dbfs, -1.5);
        assert_eq!(d.loudness_range, 11.0);
        assert_eq!(d.loudnorm_filter(), "loudnorm=I=-16:TP=-1.5:LRA=11");
        let s = LoudnormConfig::streaming();
        assert_eq!(s.integrated_lufs, -14.0);
        assert_eq!(s.true_peak_dbfs, -1.0);
    }

    #[test]
    fn default_path_with_extension() {
        let p = Path::new("/music/track.wav");
        assert_eq!(
            default_normalized_path(p),
            PathBuf::from("/music/track_normalized.wav")
        );
    }

    #[test]
    fn default_path_no_extension() {
        let p = Path::new("voice");
        assert_eq!(
            default_normalized_path(p),
            PathBuf::from("voice_normalized.wav")
        );
    }

    #[test]
    fn default_path_relative() {
        let p = Path::new("tests/test.flac");
        let got = default_normalized_path(p);
        assert!(got.ends_with("test_normalized.flac"));
    }

    #[test]
    fn extract_json_quoted_values() {
        let stderr = r#"{"input_i": "-18.2", "input_tp": "-2.1", "target_offset": "0.5"}"#;
        assert_eq!(
            extract_loudnorm_json_field(stderr, "input_i").as_deref(),
            Some("-18.2")
        );
        assert_eq!(
            extract_loudnorm_json_field(stderr, "target_offset").as_deref(),
            Some("0.5")
        );
    }

    #[test]
    fn extract_json_numeric_values() {
        let stderr = r#"{"input_i": -18.2, "input_tp": -2.1}"#;
        assert_eq!(
            extract_loudnorm_json_field(stderr, "input_i").as_deref(),
            Some("-18.2")
        );
    }
}
