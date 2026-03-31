# loudness_check

Rust tools for **measuring** loudness (`loudness`, EBU R128 via FFmpeg `ebur128`) and **normalizing** audio (`normalize`, FFmpeg `loudnorm`). Both require **[FFmpeg](https://ffmpeg.org/)** on your `PATH`.

---

## Table of contents

1. [Requirements](#requirements)
2. [Build](#build)
3. [Run with Cargo vs installed binaries](#run-with-cargo-vs-installed-binaries)
4. [`loudness` — measure loudness](#loudness--measure-loudness)
5. [`normalize` — loudness normalization](#normalize--loudness-normalization)
6. [Tests](#tests)
   - [How tests are organized](#how-tests-are-organized)
   - [Test prerequisites](#test-prerequisites)
   - [Running tests](#running-tests)
   - [Integration tests](#integration-tests)
   - [Unit tests](#unit-tests)
7. [Outputs and files](#outputs-and-files)
8. [Project layout](#project-layout)

---

## Requirements

- **Rust** (edition 2021)
- **FFmpeg** installed and available in your `PATH` (`ffmpeg -version` should work)

---

## Build

```bash
# Debug: builds all binaries
cargo build

# Optimized release binaries
cargo build --release
```

| Artifact | Debug path | Release path |
|----------|------------|--------------|
| `loudness` | `target/debug/loudness` | `target/release/loudness` |
| `normalize` | `target/debug/normalize` | `target/release/normalize` |

Build one binary only:

```bash
cargo build --bin loudness
cargo build --bin normalize
cargo build --release --bin loudness
cargo build --release --bin normalize
```

---

## Run with Cargo vs installed binaries

**Default binary** (from `Cargo.toml`): `loudness`. So `cargo run` runs `loudness` unless you pass `--bin`.

```bash
# Default: runs loudness (debug)
cargo run -- <args for loudness>

# Explicit binaries
cargo run --bin loudness -- <args>
cargo run --bin normalize -- <args>

# Release profile
cargo run --release -- <path-to-audio>
cargo run --release --bin normalize -- -i input.wav
```

**Installed path** (after `cargo build --release`):

```bash
./target/release/loudness <args>
./target/release/normalize <args>
```

**Install into Cargo bin dir** (optional):

```bash
cargo install --path .
# then, if ~/.cargo/bin is on PATH:
loudness <args>
normalize <args>
```

---

## `loudness` — measure loudness

Scans **one audio file** or **one directory** (non-recursive: only files directly in that folder).  
Supported extensions: **`m4a`**, **`mp3`**, **`flac`**, **`wav`** (case-insensitive).

On start, the binary prints the binary name (`loudness`) on stdout, then processes files.

### Syntax

```text
loudness [-V | --verbose] <file.m4a|file.mp3|file.flac|file.wav|folder>
```

- **`-V` / `--verbose`**: print the same table and progress lines to **stdout** as in the log.
- **`<path>`**: last non-flag argument is the input (file or directory).

### Examples — single file

```bash
cargo run --release -- ./music/track.wav
cargo run --release -- /absolute/path/to/song.mp3
cargo run -- -- ./tests/test.flac
./target/release/loudness ./podcast.m4a
```

### Examples — directory (all matching files, sorted)

```bash
cargo run --release -- ./album_folder/
cargo run --release -- ./tests/
./target/release/loudness ~/Audio/inbox/
```

### Examples — verbose

```bash
cargo run --release -- -V ./song.wav
cargo run --release -- --verbose ./song.wav
./target/release/loudness -V ./song.wav
```

### Report columns

Each run writes under **`LOGS/`**:

- `LOGS/<timestamp>.log`
- `LOGS/<timestamp>_loudness_report.md`

Markdown table columns:

| Song Name | I (LUFS) | LRA (LU) | TP (dBFS) | Threshold (I) | Threshold (LRA) | LRA low | LRA high |

---

## `normalize` — loudness normalization

Uses FFmpeg **`loudnorm`**. **Two-pass** is the default (analyze, then encode); **`--single-pass`** is faster but a single pass.

Default **output** if you omit **`-o` / `--output`**: same directory as the input, file name **`<stem>_normalized.<ext>`**  
(e.g. `music/song.wav` → `music/song_normalized.wav`).

### Syntax (`normalize --help`)

```text
normalize -i <INPUT> [-o <OUTPUT>] [-l LUFS] [-t TP] [--lra LRA] [--single-pass] [-- <FFMPEG_ARGS>...]
```

| Option | Meaning | Default |
|--------|---------|---------|
| `-i`, `--input` | Input file (required) | — |
| `-o`, `--output` | Output file | `<stem>_normalized.<ext>` next to input |
| `-l`, `--lufs` | Target integrated loudness (LUFS) | `-16` |
| `-t`, `--tp` | Target true peak (dBFS) | `-1.5` |
| `--lra` | Loudness range (LU) | `11` |
| `--single-pass` | Single-pass `loudnorm` | off (two-pass) |
| `--` | End of `normalize` flags; everything after is passed to **ffmpeg** (see below) | — |

### Extra FFmpeg arguments (`--`)

Anything after the **first `--` in the `normalize` argv** is forwarded to `ffmpeg`: inserted **after** `-af <loudnorm>` and **before** the output file.  
Use this for codecs, bitrate, sample rate, channels, etc.

- **Single-pass** and **two-pass pass 2** use these extras.  
- **Two-pass pass 1** (analysis) does **not** receive them.

**With `cargo run`**, you still need Cargo’s `--` before the program args, then `normalize`’s own `--` before raw ffmpeg flags:

```bash
cargo run --release --bin normalize -- -i in.wav -o out.mp3 --single-pass -- -c:a libmp3lame -b:a 192k
#                      ^ Cargo    ^ normalize flags           ^ ffmpeg flags
```

### Examples — defaults and modes

```bash
# Two-pass, default output (e.g. ./song_normalized.wav next to ./song.wav)
cargo run --release --bin normalize -- -i ./song.wav

# Two-pass, explicit output
cargo run --release --bin normalize -- -i ./song.wav -o /tmp/normalized.wav

# Single-pass, default output path
cargo run --release --bin normalize -- -i ./song.wav --single-pass

# Single-pass, explicit output
cargo run --release --bin normalize -- -i ./in.mp3 -o ./out.wav --single-pass
```

### Examples — loudness targets (EBU / streaming-style)

```bash
# EBU-style defaults are already -16 / -1.5 / 11; override explicitly:
cargo run --release --bin normalize -- -i track.wav -l -16 -t -1.5 --lra 11

# Louder integrated target (e.g. streaming -14 LUFS style)
cargo run --release --bin normalize -- -i track.wav -l -14 -t -1.0 --lra 11 --single-pass
```

### Examples — FFmpeg passthrough (codec, rate, channels)

```bash
# PCM WAV, 48 kHz
cargo run --release --bin normalize -- -i a.wav -o b.wav --single-pass -- -ar 48000

# MP3 output with explicit encoder
cargo run --release --bin normalize -- -i a.wav -o b.mp3 --single-pass -- -c:a libmp3lame -b:a 192k

# FLAC output
cargo run --release --bin normalize -- -i a.wav -o b.flac --single-pass -- -c:a flac

# Stereo downmix hint (if your ffmpeg build supports the filter; often used in ffmpeg chains)
# (example: you can pass any ffmpeg output options that apply before the final file name)
cargo run --release --bin normalize -- -i a.wav -o b.wav --single-pass -- -ac 2
```

### Examples — direct binary (no Cargo)

```bash
./target/release/normalize -i ./input.wav --single-pass
./target/release/normalize -i ./input.wav -o ./out.mp3 --single-pass -- -c:a libmp3lame -b:a 192k
```

### Help and version

```bash
cargo run --bin normalize -- --help
cargo run --bin normalize -- --version
./target/release/normalize --help
```

---

## Tests

### How tests are organized

The project uses two layers:

| Layer | Location | What it exercises |
|-------|----------|-------------------|
| **Integration tests** | `tests/integration.rs` | End-to-end: each test runs **`cargo`** to build/execute **`loudness`** or **`normalize`** with real **FFmpeg** and fixture files under `tests/`. |
| **Unit tests** | `src/bin/normalize.rs` (`#[cfg(test)]`) | Pure logic only: default paths, JSON field parsing for `loudnorm` analysis, and `LoudnormConfig` defaults—**no** FFmpeg and **no** subprocess. |

There is no library crate, so there are no doc tests. The **`ffmpeg-usage`** binary (`src/main.rs`) has no tests.

### Test prerequisites

- Run commands from the **repository root** (where `Cargo.toml` lives).
- **FFmpeg** must be on `PATH` for integration tests (they invoke the real binaries, which call `ffmpeg`).
- Integration tests spawn **`cargo run`**; the first run may compile targets before executing—expect slower runs than typical unit-only crates.

### Running tests

```bash
# All tests (unit + integration)
cargo test

# Only the integration test binary
cargo test --test integration

# Only unit tests embedded in the normalize binary
cargo test --bin normalize

# Quiet output
cargo test -q

# One integration test by name (example)
cargo test --test integration test_single_file_m4a
```

### Integration tests

File: **`tests/integration.rs`** (12 tests).

**`loudness` (default `cargo run`)** — uses `tests/test.*` fixtures and sometimes `tests/` as a folder:

| Test | What it checks |
|------|----------------|
| `test_single_file_m4a` / `mp3` / `flac` | Single-file path for each supported extension exits successfully. |
| `test_folder_input` | Directory argument processes non-recursive contents. |
| `test_log_file_created` | A run creates **`LOGS/`** with at least one file. |
| `test_true_peak_extraction` | Markdown report contains a **TP (dBFS)** column with a non-`N/A` value. |

**`normalize`** — **`--bin normalize`**, **`--single-pass`** unless noted; outputs often go under **`target/`** or next to fixtures:

| Test | What it checks |
|------|----------------|
| `normalize_cli_single_pass_wav` | WAV in → normalized WAV under `target/`. |
| `normalize_cli_single_pass_mp3_input` / `m4a_input` | Compressed inputs → **WAV** under `target/` (avoids flaky re-encode to MP3/AAC). |
| `normalize_cli_accepts_one_input_file` | Explicit `-o` path works. |
| `normalize_cli_accepts_folder_input` | `-i` is a directory; batch writes `<stem>_normalized.<ext>` beside sources. |
| `normalize_cli_single_pass_default_output_path` | Omitting `-o` writes `tests/test_normalized.wav` next to the input. |

### Unit tests

Six tests live inside **`src/bin/normalize.rs`** under `#[cfg(test)]`: they validate **`default_normalized_path`**, **`extract_loudnorm_json_field`**, and **`LoudnormConfig`** / **`loudnorm_filter()`** without running FFmpeg.

---

## Outputs and files

| Tool | Where | What |
|------|--------|------|
| `loudness` | `LOGS/` | Timestamped `.log` and `*_loudness_report.md`; stdout also prints report path and log path |
| `normalize` | User-chosen or `<stem>_normalized.<ext>` | Normalized media file |

The repo **ignores** `LOGS/` and `tests/*_normalized.*` via `.gitignore` (generated artifacts).

---

## Project layout

| Path | Role |
|------|------|
| `src/ffmpeg_tools.rs` | Shared FFmpeg presence check and supported-audio file listing |
| `src/bin/loudness.rs` | `loudness` binary (EBU R128 measure + `LOGS/`) |
| `src/bin/normalize.rs` | `normalize` binary (clap + `--` ffmpeg passthrough) |
| `src/main.rs` | `ffmpeg-usage` binary (prints CLI summaries) |
| `tests/integration.rs` | Integration tests |

---

## Rust crate

This package is **binary-only** (no `lib.rs`). Shared helpers live in `src/ffmpeg_tools.rs` and are included by each binary that needs them.
