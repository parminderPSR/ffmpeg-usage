//! Integration tests: `loudness` and `normalize` CLIs (require ffmpeg, run from package root).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// --- loudness ---

#[test]
fn test_single_file_m4a() {
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/test.m4a"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
}

#[test]
fn test_single_file_mp3() {
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/test.mp3"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
}

#[test]
fn test_single_file_flac() {
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/test.flac"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
}

#[test]
fn test_folder_input() {
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
}

#[test]
fn test_log_file_created() {
    let _ = fs::remove_dir_all(manifest_dir().join("LOGS"));
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/test.wav"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
    let logs = manifest_dir().join("LOGS");
    assert!(logs.metadata().is_ok());
    let entries: Vec<_> = fs::read_dir(&logs).unwrap().collect();
    assert!(!entries.is_empty(), "No log files found in LOGS directory");
}

#[test]
fn test_true_peak_extraction() {
    let _ = fs::remove_dir_all(manifest_dir().join("LOGS"));
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args(["run", "--", "tests/test.wav"])
        .status()
        .expect("Failed to run cargo");
    assert!(status.success());
    let mut entries: Vec<_> = fs::read_dir(manifest_dir().join("LOGS"))
        .unwrap()
        .collect();
    entries.sort_by_key(|e| e.as_ref().unwrap().metadata().unwrap().modified().unwrap());
    let report_path = entries
        .iter()
        .filter_map(|e| e.as_ref().ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("loudness_report.md")
        })
        .map(|e| e.path())
        .expect("No report file found");
    let content = fs::read_to_string(report_path).expect("Failed to read report");
    assert!(
        content.contains("TP (dBFS)"),
        "Report does not contain TP column"
    );
    let tp_values: Vec<&str> = content
        .lines()
        .filter(|l| l.starts_with("| ") && !l.contains("---"))
        .map(|l| l.split('|').map(|s| s.trim()).collect::<Vec<_>>())
        .filter(|cols| cols.len() > 8)
        .map(|cols| cols[8])
        .collect();
    assert!(
        tp_values.iter().any(|&v| v != "N/A"),
        "No TP value extracted"
    );
}

// --- normalize CLI (ffmpeg) ---

fn assert_normalize_single_pass_output(input_rel: &str, target_output_rel: &str) {
    let out = manifest_dir().join("target").join(target_output_rel);
    let _ = fs::remove_file(&out);
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args([
            "run",
            "--bin",
            "normalize",
            "--",
            "-i",
            input_rel,
            "-o",
            out.to_str().expect("utf8 path"),
            "--single-pass",
        ])
        .status()
        .unwrap_or_else(|e| panic!("cargo run normalize for {}: {}", input_rel, e));
    assert!(
        status.success(),
        "normalize CLI failed for input {}",
        input_rel
    );
    assert!(out.is_file(), "expected output at {:?}", out);
    assert!(
        fs::metadata(&out).map(|m| m.len() > 0).unwrap_or(false),
        "output empty: {:?}",
        out
    );
}

#[test]
fn normalize_cli_single_pass_wav() {
    assert_normalize_single_pass_output("tests/test.wav", "normalize_cli_test.wav");
}

/// MP3 input; WAV output avoids flaky MP3 re-encode after `loudnorm` in CI/local ffmpeg builds.
#[test]
fn normalize_cli_single_pass_mp3_input() {
    assert_normalize_single_pass_output("tests/test.mp3", "normalize_cli_from_mp3.wav");
}

/// M4A input; WAV output avoids AAC encoder issues after single-pass `loudnorm`.
#[test]
fn normalize_cli_single_pass_m4a_input() {
    assert_normalize_single_pass_output("tests/test.m4a", "normalize_cli_from_m4a.wav");
}

/// Single input file (`-i` file) produces output; documents one-file acceptance.
#[test]
fn normalize_cli_accepts_one_input_file() {
    assert_normalize_single_pass_output("tests/test.wav", "normalize_one_file_out.wav");
}

/// `-i` pointing at a directory: every m4a/mp3/flac/wav in that directory (non-recursive) is normalized beside the source.
#[test]
fn normalize_cli_accepts_folder_input() {
    let root = manifest_dir().join("target/normalize_folder_batch_test");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir batch test");
    let src = manifest_dir().join("tests/test.wav");
    let staged = root.join("clip.wav");
    fs::copy(&src, &staged).expect("copy fixture");
    let expected_out = root.join("clip_normalized.wav");
    let _ = fs::remove_file(&expected_out);

    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args([
            "run",
            "--bin",
            "normalize",
            "--",
            "-i",
            root.to_str().expect("utf8"),
            "--single-pass",
        ])
        .status()
        .expect("cargo run normalize folder");
    assert!(
        status.success(),
        "normalize batch folder should exit 0"
    );
    assert!(
        expected_out.is_file(),
        "expected {:?} from folder batch",
        expected_out
    );
    assert!(
        fs::metadata(&expected_out).map(|m| m.len() > 0).unwrap_or(false),
        "batch output empty"
    );
    let _ = fs::remove_dir_all(&root);
}

/// Default output path: same directory as input, `<stem>_normalized.<ext>` (e.g. `tests/test.wav` → `tests/test_normalized.wav`).
#[test]
fn normalize_cli_single_pass_default_output_path() {
    let default_out = manifest_dir().join("tests/test_normalized.wav");
    let _ = fs::remove_file(&default_out);
    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args([
            "run",
            "--bin",
            "normalize",
            "--",
            "-i",
            "tests/test.wav",
            "--single-pass",
        ])
        .status()
        .expect("cargo run normalize");
    assert!(status.success(), "normalize CLI should exit 0");
    assert!(
        default_out.is_file(),
        "expected default output at {:?}",
        default_out
    );
    assert!(
        fs::metadata(&default_out).map(|m| m.len() > 0).unwrap_or(false),
        "default output file empty"
    );
    let _ = fs::remove_file(&default_out);
}
