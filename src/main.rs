//! Prints bundled CLI tools and their default options (no analysis or encoding).

fn main() {
    println!("loudness");
    println!("  Usage: loudness [-v|--verbose] <file.m4a|folder>  (-V is --version)");
    println!("  Defaults: analyze EBU R128 via FFmpeg; write LOGS/ and a Markdown report.");
    println!();
    println!("normalize");
    println!("  Usage: normalize -i <file|dir> [-o <file>] [-l LUFS] [-t TP] [--lra LU] [--single-pass] [--] [ffmpeg args...]");
    println!("  Defaults: -l -16 (LUFS), -t -1.5 (dBFS true peak), --lra 11, two-pass loudnorm");
    println!("  Single file without -o: <stem>_normalized.<ext> beside the input; directory -i: same per file");
}
