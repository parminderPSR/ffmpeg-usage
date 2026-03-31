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

/// Nested directories only (no audio at the root): recursion must find files in subfolders.
#[test]
fn test_folder_input_recursive() {
    let root = manifest_dir().join("target/loudness_recursive_fixture");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("nested/deep")).expect("mkdir nested");
    let src = manifest_dir().join("tests/test.wav");
    fs::copy(&src, root.join("nested/one.wav")).expect("copy nested wav");
    fs::copy(&src, root.join("nested/deep/two.wav")).expect("copy deep wav");

    let status = Command::new("cargo")
        .current_dir(manifest_dir())
        .args([
            "run",
            "--bin",
            "loudness",
            "--",
            root.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("cargo run loudness recursive");
    assert!(
        status.success(),
        "loudness should exit 0 for nested-only folder"
    );

    // Pick the log from this run (do not assume newest *.md — parallel tests also write LOGS/).
    let logs_dir = manifest_dir().join("LOGS");
    let mut log_files: Vec<_> = fs::read_dir(&logs_dir)
        .expect("LOGS")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .collect();
    log_files.sort_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()));
    log_files.reverse();

    let log_for_run = log_files
        .into_iter()
        .map(|e| fs::read_to_string(e.path()).expect("read log"))
        .find(|s| {
            s.contains("loudness_recursive_fixture")
                && s.lines().any(|l| l.starts_with("target:"))
        })
        .expect("log for recursive fixture run");

    assert!(
        log_for_run.contains("file_count: 2"),
        "expected two nested wav files; log excerpt:\n{}",
        log_for_run
            .lines()
            .take(12)
            .collect::<Vec<_>>()
            .join("\n")
    );

    let _ = fs::remove_dir_all(&root);
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

/// `-i` pointing at a directory: every m4a/mp3/flac/wav in that directory is normalized beside the source.
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

/// Nested folders only: batch must find wav files in subdirs and write outputs beside each source.
#[test]
fn normalize_cli_accepts_folder_input_recursive() {
    let root = manifest_dir().join("target/normalize_recursive_fixture");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("nested/deep")).expect("mkdir nested");
    let src = manifest_dir().join("tests/test.wav");
    fs::copy(&src, root.join("nested/one.wav")).expect("copy nested wav");
    fs::copy(&src, root.join("nested/deep/two.wav")).expect("copy deep wav");

    let out_one = root.join("nested/one_normalized.wav");
    let out_two = root.join("nested/deep/two_normalized.wav");
    let _ = fs::remove_file(&out_one);
    let _ = fs::remove_file(&out_two);

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
        .expect("cargo run normalize recursive folder");
    assert!(
        status.success(),
        "normalize batch recursive folder should exit 0"
    );
    assert!(
        out_one.is_file(),
        "expected {:?} beside nested input",
        out_one
    );
    assert!(
        out_two.is_file(),
        "expected {:?} beside deep input",
        out_two
    );
    assert!(
        fs::metadata(&out_one).map(|m| m.len() > 0).unwrap_or(false),
        "nested output empty"
    );
    assert!(
        fs::metadata(&out_two).map(|m| m.len() > 0).unwrap_or(false),
        "deep output empty"
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
