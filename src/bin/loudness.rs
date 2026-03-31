//! `loudness` — EBU R128 loudness analysis via FFmpeg `ebur128`; writes LOGS/ and a Markdown report.

#[path = "../ffmpeg_tools.rs"]
mod ffmpeg_tools;
use chrono::Local;
use clap::Parser;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "loudness",
    version,
    about = "EBU R128 loudness measurement (FFmpeg ebur128); writes LOGS/ and a Markdown report.",
    after_help = r#"Suggestions:
  • ffmpeg must be on your PATH (this tool shells out to ffmpeg).
  • Pass one audio file or one directory. Supported extensions: m4a, mp3, flac, wav (case-insensitive).
  • A directory is scanned non-recursively: only files directly in that folder are analyzed.
  • Use -v / --verbose to mirror the Markdown table and per-file metrics on stdout (-V is --version).
  • Each run creates timestamped files under LOGS/: a .log and a *_loudness_report.md.

Examples:
  loudness music/track.wav
  loudness -v podcast.m4a
  loudness --verbose ~/Audio/inbox/
  loudness ./tests/
  cargo run --release -- ./album/
"#
)]
struct Args {
    /// Input audio file, or a directory containing m4a/mp3/flac/wav (non-recursive)
    #[arg(value_name = "PATH")]
    target: String,

    /// Print the metrics table and per-file lines to stdout (same as in the log/report)
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// CLI entry: scan files, write logs and Markdown report. Exits the process on usage/ffmpeg errors.
fn run(args: Args) -> io::Result<()> {
    let target = args.target;
    let verbose = args.verbose;
    let mut files = Vec::new();
    let path = Path::new(&target);
    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ffmpeg_tools::is_supported_audio_extension(ext) {
                files.push(target.clone());
            }
        }
    } else if path.is_dir() {
        files = ffmpeg_tools::list_supported_audio_files_in_dir(path)?
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
    } else {
        eprintln!(
            "Input must be an audio file (m4a, mp3, flac, wav) or a folder containing such files."
        );
        std::process::exit(1);
    }
    ffmpeg_tools::require_ffmpeg();

    let logs_dir = Path::new("LOGS");
    if !logs_dir.exists() {
        fs::create_dir_all(logs_dir)?;
    }
    let started = Local::now();
    // Unique per run (minute-level names could overwrite on fast repeat runs).
    let log_prefix = format!(
        "{}_{:03}",
        started.format("%Y_%m_%d_%H_%M_%S"),
        started.timestamp_subsec_millis()
    );
    let log_filename = format!("LOGS/{}.log", log_prefix);
    let log_handle = File::create(&log_filename)?;
    let mut log_file = BufWriter::new(log_handle);

    let md_file = format!("LOGS/{}_loudness_report.md", log_prefix);
    let mut file = File::create(&md_file)?;
    let header = "| Song Name | I (LUFS) | LRA (LU) | TP (dBFS) | Threshold (I) | Threshold (LRA) | LRA low | LRA high |";
    let divider = "|-----------|----------|----------|-----------|---------------|-----------------|---------|----------|";

    writeln!(log_file, "loudness")?;
    writeln!(
        log_file,
        "started: {}",
        started.format("%Y-%m-%d %H:%M:%S%.3f %z")
    )?;
    writeln!(log_file, "target: {}", target)?;
    writeln!(log_file, "file_count: {}", files.len())?;
    writeln!(log_file, "---")?;

    writeln!(file, "{}", header)?;
    writeln!(file, "{}", divider)?;
    writeln!(log_file, "{}", header)?;
    writeln!(log_file, "{}", divider)?;
    if verbose {
        println!("{}", header);
        println!("{}", divider);
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut ok_count = 0u32;
    let mut fail_count = 0u32;
    for f in &files {
        writeln!(log_file, "Processing: {}", f)?;
        log_file.flush()?;
        if verbose {
            println!("Processing: {}", f);
        }
        match process_file(f) {
            Ok(row) => {
                let log_line = format!(
                    "I: {} | LRA: {} | TP: {} | Threshold (I): {} | Threshold (LRA): {} | LRA low: {} | LRA high: {}",
                    row[1], row[2], row[3], row[4], row[5], row[6], row[7]
                );
                writeln!(log_file, "  status: OK")?;
                writeln!(log_file, "  {}", log_line)?;
                if verbose {
                    println!("{}", log_line);
                }
                rows.push(row);
                ok_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to process {}: {}", f, e);
                writeln!(log_file, "  status: ERROR")?;
                writeln!(log_file, "  {}", e)?;
                if verbose {
                    println!("Failed to process {}: {}", f, e);
                }
                fail_count += 1;
            }
        }
    }
    for row in rows {
        writeln!(file, "| {} |", row.join(" | "))?;
    }
    file.flush()?;

    let finished = Local::now();
    writeln!(log_file, "---")?;
    writeln!(
        log_file,
        "finished: {}",
        finished.format("%Y-%m-%d %H:%M:%S%.3f %z")
    )?;
    writeln!(
        log_file,
        "summary: {} ok, {} failed",
        ok_count, fail_count
    )?;
    writeln!(log_file, "report: {}", md_file)?;
    log_file.flush()?;

    println!("Markdown report generated: {}", md_file);
    println!("Log: {}", log_filename);
    Ok(())
}

/// Parses FFmpeg `ebur128` summary lines from stderr into metric strings (or `"N/A"`).
fn extract_loudness_info(ffmpeg_output: &str) -> Vec<String> {
    use regex::Regex;
    let mut matches = Vec::new();
    // I:         -16.0 LUFS
    let re_i = Regex::new(r"(?m)^\s*I:\s*(-?\d+\.\d+) LUFS").unwrap();
    matches.push(
        re_i.captures(ffmpeg_output)
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // LRA:         2.7 LU
    let re_lra = Regex::new(r"(?m)^\s*LRA:\s*(-?\d+\.\d+) LU").unwrap();
    matches.push(
        re_lra
            .captures(ffmpeg_output)
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // TP:       -3.2 dBFS (allow any leading whitespace and match at line start)
    let re_tp = Regex::new(r"(?m)^\s*Peak:\s*(-?\d+\.\d+) dBFS").unwrap();
    matches.push(
        re_tp
            .captures_iter(ffmpeg_output)
            .last()
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // Threshold (I): -26.0 LUFS (first occurrence)
    let re_threshold = Regex::new(r"(?m)^\s*Threshold:\s*(-?\d+\.\d+) LUFS").unwrap();
    let mut threshold_iter = re_threshold.captures_iter(ffmpeg_output);
    matches.push(
        threshold_iter
            .next()
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // Threshold (LRA): -36.0 LUFS (second occurrence)
    matches.push(
        threshold_iter
            .next()
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // LRA low:   -17.0 LUFS
    let re_lra_low = Regex::new(r"(?m)^\s*LRA low:\s*(-?\d+\.\d+) LUFS").unwrap();
    matches.push(
        re_lra_low
            .captures(ffmpeg_output)
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    // LRA high:  -14.3 LUFS
    let re_lra_high = Regex::new(r"(?m)^\s*LRA high:\s*(-?\d+\.\d+) LUFS").unwrap();
    matches.push(
        re_lra_high
            .captures(ffmpeg_output)
            .and_then(|c| c.get(1))
            .map_or("N/A".to_string(), |m| m.as_str().to_string()),
    );
    matches
}

/// Runs FFmpeg with `ebur128` on `filepath` and returns one row: file name plus parsed metrics.
fn process_file(filepath: &str) -> io::Result<Vec<String>> {
    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(filepath)
        .arg("-filter_complex")
        .arg("ebur128=framelog=quiet:peak=true")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let info = extract_loudness_info(&stderr);
    let mut row = vec![Path::new(filepath)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string()];
    row.extend(info);
    Ok(row)
}

fn main() -> std::io::Result<()> {
    let cli = Args::parse();
    println!("{}", env!("CARGO_BIN_NAME"));
    run(cli)
}
