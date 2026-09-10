/*
   Copyright (C) 2026 Kamila Szewczyk <k@iczelia.net> on the behalf of the Saarland University.

   This program is free software; you can redistribute it and/or modify
   it under the terms of the GNU General Public License as published by
   the Free Software Foundation; either version 3 of the License, or
   (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU General Public License for more details.

   You should have received a copy of the GNU General Public License
   along with this program. If not, see <http://www.gnu.org/licenses/>.
*/

mod cmd;

use clap::{Parser, Subcommand};
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "hecate",
    version = "1.0 - a referential and reference-free sequence codec.\n\
        Written by Kamila Szewczyk on the behalf of the Saarland University.\n\
        Licensed under the terms of the GNU GPLv3 License. See LICENSE for details.",
    about = "High-performance genomic sequence codec"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Input file (or positional)
    #[arg(global = true)]
    pub file: Option<PathBuf>,

    /// Output file
    #[arg(short, long, global = true)]
    pub output: Option<PathBuf>,

    /// Force output to stdout
    #[arg(short = 'c', long = "stdout", global = true)]
    pub to_stdout: bool,

    /// Overwrite existing output
    #[arg(short, long, global = true)]
    pub force: bool,

    /// Print compression statistics
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-error output
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    // Short-form flags (when no subcommand)
    /// Encode mode
    #[arg(short = 'e', long = "encode")]
    flag_encode: bool,

    /// Decode mode
    #[arg(short = 'd', long = "decode")]
    flag_decode: bool,

    /// List mode
    #[arg(short = 'l', long = "list")]
    flag_list: bool,

    /// Verify mode
    #[arg(long = "verify")]
    flag_verify: bool,

    // Encode options
    /// 2-bit encoding (A/C/G/T only)
    #[arg(short = '2', long = "two-bit", global = true)]
    pub two_bit: bool,

    /// 4-bit encoding (full IUPAC)
    #[arg(short = '4', long = "four-bit", global = true)]
    pub four_bit: bool,

    /// 8-bit encoding (raw bytes)
    #[arg(short = '8', long = "eight-bit", global = true)]
    pub eight_bit: bool,

    /// Discard case information
    #[arg(short = 'C', long = "ignore-case", global = true)]
    pub ignore_case: bool,

    /// Merge case into nucleotide stream (8-bit only)
    #[arg(short = 'J', long = "joint-case", global = true)]
    pub joint_case: bool,

    /// Treat input as FASTA
    #[arg(short = 'A', long = "fasta", global = true)]
    pub fasta: bool,

    /// Treat input as FASTQ
    #[arg(short = 'Q', long = "fastq", global = true)]
    pub fastq: bool,

    /// Compression methods, comma-separated.
    /// Formats: "zstd[:level[:window_log]]", "xz[:preset[e]]" or
    /// "xz:preset=Ne,dict=SIZE,lc=N,pb=N", "bwt[:block_mib]", "markov",
    /// "bmarkov", "vmarkov", "rans".
    /// Example: "zstd:3" or "zstd:3,markov" or "xz:9e".
    /// When omitted, data is stored uncompressed.
    #[arg(short = 'M', long = "methods", global = true, verbatim_doc_comment)]
    pub methods: Option<String>,

    /// Per-stream compression pattern, one character per stream.
    /// Each character is a digit (method index) or "_" for uncompressed.
    /// FASTA streams: CTRL(0), NUC(1), CASE(2), HDR(3), EXTRA(4).
    /// FASTQ adds: QUALITY(5).
    /// Example: "00_0_" compresses all but CASE and EXTRA.
    /// When omitted and markov/bmarkov is in -M, it is assigned to NUC
    /// and any other method covers the remaining streams.
    #[arg(short = 'P', long = "pattern", global = true, verbatim_doc_comment)]
    pub pattern: Option<String>,

    /// Enable indexing (needs tier: "full" or "block")
    #[arg(short = 'I', long = "index", global = true, num_args = 0..=1, require_equals = true, default_missing_value = "full")]
    pub index: Option<String>,

    /// FASTA line width for decode (default 80, 0 = no wrapping)
    #[arg(short = 'w', long = "line-width", default_value = "80", global = true)]
    pub line_width: usize,

    /// Slice specification (repeatable).
    /// Formats: "name", "name:start:end", "/regex/", "/regex/:start:end".
    /// Coordinates are 0-based, half-open [start, end).
    #[arg(short = 'S', long = "slice", global = true, verbatim_doc_comment)]
    pub slice: Vec<String>,

    /// Strip quality data
    #[arg(long = "strip-quality", global = true)]
    pub strip_quality: bool,

    /// Invert case
    #[arg(long = "invert-case", global = true)]
    pub invert_case: bool,

    /// Illumina FASTQ mode: tokenize headers, auto-rANS for quality/coords
    #[arg(short = 'L', long = "illumina", global = true)]
    pub illumina: bool,

    /// Quality score binning: "illumina" or "bin:N"
    #[arg(short = 'B', long = "quality-bin", global = true)]
    pub quality_bin: Option<String>,

    /// Disable memory-mapped input
    #[arg(long = "no-mmap", global = true)]
    pub no_mmap: bool,

    /// Pre/post filter command
    #[arg(long = "filter", global = true)]
    pub filter: Option<String>,

    /// Reference .he file for referential compression
    #[arg(short = 'R', long = "reference", global = true)]
    pub reference: Option<PathBuf>,

    /// Referential encode mode
    #[arg(short = 'E', long = "ref-encode")]
    flag_ref_encode: bool,

    /// Referential decode mode
    #[arg(short = 'D', long = "ref-decode")]
    flag_ref_decode: bool,

    /// Grep for a nucleotide pattern in a .he file.
    /// Syntax: /pattern/flags where # is a single-base wildcard.
    /// Flags: i = case-insensitive, g = global (all matches).
    #[arg(short = 'G', long = "grep", global = true, verbatim_doc_comment)]
    pub grep: Option<String>,

    /// Neighbourhood size (bases of context around each grep match)
    #[arg(
        short = 'n',
        long = "neighbourhood",
        default_value = "0",
        global = true
    )]
    pub neighbourhood: usize,

    /// Write per-base markov probabilities to FILE (xz -9, 5 decimals, one per
    /// line). Requires the markov codec on the NUC stream; encode-only.
    #[arg(long = "mprob", value_name = "FILE", global = true)]
    pub mprob: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Encode FASTA/FASTQ to Hecate format
    Encode,
    /// Decode Hecate to FASTA/FASTQ
    Decode,
    /// Extract sequences/regions from indexed Hecate
    Slice,
    /// List sequences in a Hecate file
    List,
    /// Re-encode a Hecate file with different settings
    Recode,
    /// Verify integrity of a Hecate file
    Verify,
    /// Show file metadata and per-stream compression statistics
    Stats,
    /// Concatenate Hecate files without recompression
    Cat {
        /// Input files to concatenate
        #[arg(short = 'i', long = "input", required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,
    },
    /// Referential encode: create diff against a reference .he file
    RefEncode,
    /// Referential decode: reconstruct from diff + reference .he file
    RefDecode,
    /// Search for nucleotide patterns in a Hecate file
    Query,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("hecate: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> io::Result<()> {
    let mode = resolve_mode(&cli)?;
    match mode {
        Mode::Encode => cmd::encode::cmd_encode(&cli),
        Mode::Decode => cmd::decode::cmd_decode(&cli),
        Mode::Slice => cmd::slice::cmd_slice(&cli),
        Mode::List => cmd::list::cmd_list(&cli),
        Mode::Recode => cmd::recode::cmd_recode(&cli),
        Mode::Verify => cmd::verify::cmd_verify(&cli),
        Mode::Stats => cmd::stats::cmd_stats(&cli),
        Mode::Cat => {
            let inputs = match cli.command {
                Some(Command::Cat { inputs }) => inputs,
                _ => unreachable!(),
            };
            cmd::cat::cmd_cat(
                cli.output.as_ref(),
                cli.force,
                cli.verbose,
                cli.no_mmap,
                inputs,
            )
        }
        Mode::RefEncode => cmd::ref_encode::cmd_ref_encode(&cli),
        Mode::RefDecode => cmd::ref_decode::cmd_ref_decode(&cli),
        Mode::Query => cmd::query::cmd_query(&cli),
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Encode,
    Decode,
    Slice,
    List,
    Recode,
    Verify,
    Stats,
    Cat,
    RefEncode,
    RefDecode,
    Query,
}

fn resolve_mode(cli: &Cli) -> io::Result<Mode> {
    if let Some(ref cmd) = cli.command {
        return Ok(match cmd {
            Command::Encode => Mode::Encode,
            Command::Decode => Mode::Decode,
            Command::Slice => Mode::Slice,
            Command::List => Mode::List,
            Command::Recode => Mode::Recode,
            Command::Verify => Mode::Verify,
            Command::Stats => Mode::Stats,
            Command::Cat { .. } => Mode::Cat,
            Command::RefEncode => Mode::RefEncode,
            Command::RefDecode => Mode::RefDecode,
            Command::Query => Mode::Query,
        });
    }
    if cli.grep.is_some() {
        return Ok(Mode::Query);
    }
    if cli.flag_ref_encode {
        return Ok(Mode::RefEncode);
    }
    if cli.flag_ref_decode {
        return Ok(Mode::RefDecode);
    }
    if cli.flag_encode {
        return Ok(Mode::Encode);
    }
    // Slice before decode: -S implies Slice mode even with -d.
    if !cli.slice.is_empty() {
        return Ok(Mode::Slice);
    }
    if cli.flag_decode {
        return Ok(Mode::Decode);
    }
    if cli.flag_list {
        return Ok(Mode::List);
    }
    if cli.flag_verify {
        return Ok(Mode::Verify);
    }
    // Auto-detect from file extension
    if let Some(ref path) = cli.file {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "he" => return Ok(Mode::Decode),
                "fa" | "fasta" | "fna" => return Ok(Mode::Encode),
                "fq" | "fastq" => return Ok(Mode::Encode),
                _ => {}
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "cannot determine mode - use a subcommand (encode/decode/slice/list) or -e/-d/-l",
    ))
}
