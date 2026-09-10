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

// Core CLI behavior tests: auto-detection, stdout, verbose, errors, subcommands,
// flag combinations, --no-mmap paths, and error coverage.
//
// Round-trip tests       -> tests/roundtrip.rs
// Slice / index tests    -> tests/slice.rs
// Grep (query) tests     -> tests/grep.rs
// Command tests (list, recode, verify, stats, cat, strip-quality, invert-case)
//                        -> tests/commands.rs

mod common;

use common::{data_path, hecate, normalize_fasta};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Auto-detection from extension
// ---------------------------------------------------------------------------

#[test]
fn auto_detect_fasta_from_extension() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    // Auto-detect encode from .fa extension
    let status = hecate()
        .args(["-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Auto-detect decode from .he extension
    let status = hecate()
        .args(["-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// Stdout output
// ---------------------------------------------------------------------------

#[test]
fn decode_to_stdout() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-d", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// Verbose output
// ---------------------------------------------------------------------------

#[test]
fn verbose_flag_outputs_stats() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let output = hecate()
        .args(["-e", "-2", "--verbose", &data_path("small.fa"), "-o"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("encode:"),
        "verbose should print stats to stderr"
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn error_missing_input() {
    let output = hecate()
        .args(["-e", "-2", "/nonexistent/file.fa", "-o", "/tmp/out.he"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn error_no_mode() {
    let output = hecate().arg("/tmp/somefile.xyz").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn error_exists_without_force() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // First encode succeeds
    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Second encode without --force should fail
    let output = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("exists"), "should warn about existing file");
}

#[test]
fn force_overwrites() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // With --force, should succeed
    let status = hecate()
        .args(["-e", "-2", "-f", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());
}

// ---------------------------------------------------------------------------
// Subcommand syntax
// ---------------------------------------------------------------------------

#[test]
fn subcommand_encode_decode() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .arg("encode")
        .args(["-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .arg("decode")
        .args(["-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// Combination tests: multiple flags
// ---------------------------------------------------------------------------

/// Decode with --invert-case and --strip-quality simultaneously.
#[test]
fn combo_invert_case_strip_quality() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("reads.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-d", "--invert-case", "--strip-quality", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "combo invert+strip failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    // Should be FASTA (quality stripped)
    assert!(decoded.starts_with('>'), "should be FASTA format");
    assert!(!decoded.contains('+'), "no FASTQ quality separator");
    // Case should be inverted (original is uppercase ACGT -> lowercase acgt)
    for line in decoded.lines() {
        if !line.starts_with('>') && !line.is_empty() {
            for ch in line.chars() {
                assert!(
                    ch.is_ascii_lowercase(),
                    "expected lowercase after invert, got '{}'",
                    ch
                );
            }
        }
    }
}

/// Slice a single sequence with --invert-case (indexed path).
#[test]
fn combo_slice_invert_case_indexed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice gene1 with --invert-case
    let output = hecate()
        .args(["--invert-case", "-S", "/gene1/", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "slice+invert failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    // Original gene1: "ACGTacgtACGT" -> inverted: "acgtACGTacgt"
    let seq_line: &str = decoded.lines().find(|l| !l.starts_with('>')).unwrap();
    assert_eq!(seq_line, "acgtACGTacgt", "case should be inverted");
}

/// Slice a sub-range with --invert-case (indexed path).
#[test]
fn combo_slice_subrange_invert_case() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice gene1 bases 5-8 (1-based) with --invert-case
    // Original: ACGTacgtACGT, bases 5-8 = "acgt"
    // After invert: "ACGT"
    let output = hecate()
        .args(["--invert-case", "-S", "/gene1/:5:8", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "subrange slice+invert failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    let seq_line: &str = decoded.lines().find(|l| !l.starts_with('>')).unwrap();
    assert_eq!(
        seq_line, "ACGT",
        "inverted sub-range should be all uppercase"
    );
}

/// Recode a single sequence extracted by slice.
#[test]
fn combo_recode_single_sequence() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("multi.he");
    let sliced_he = tmp.path().join("sliced.he");
    let fa = tmp.path().join("sliced.fa");

    // Encode multi-sequence genome with indexing
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode: change compression (from zstd:3 to no compression)
    let status = hecate()
        .args(["recode", "-M", "zstd:1"])
        .arg(&he)
        .args(["-o"])
        .arg(&sliced_he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode the re-encoded file
    let status = hecate()
        .args(["-d", "-w", "80"])
        .arg(&sliced_he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "recoded file should roundtrip exactly");
}

/// Recode with --invert-case then decode should give inverted output.
#[test]
fn combo_recode_invert_then_decode() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");
    let inv_he = tmp.path().join("inverted.he");
    let fa = tmp.path().join("inverted.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode with invert
    let status = hecate()
        .args(["recode", "--invert-case"])
        .arg(&he)
        .args(["-o"])
        .arg(&inv_he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode
    let status = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&inv_he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let decoded = fs::read_to_string(&fa).unwrap();
    // gene1: "ACGTacgtACGT" -> inverted: "acgtACGTacgt"
    assert!(
        decoded.contains("acgtACGTacgt"),
        "gene1 case should be inverted"
    );
    // gene2: "acgtacgtacgt" -> inverted: "ACGTACGTACGT"
    assert!(
        decoded.contains("ACGTACGTACGT"),
        "gene2 case should be inverted"
    );
}

/// Recode with --strip-quality converts FASTQ to FASTA, then decode.
#[test]
fn combo_recode_strip_quality_then_decode() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("reads.he");
    let stripped_he = tmp.path().join("stripped.he");
    let fa = tmp.path().join("stripped.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode stripping quality
    let status = hecate()
        .args(["recode", "--strip-quality"])
        .arg(&he)
        .args(["-o"])
        .arg(&stripped_he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&stripped_he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let decoded = fs::read_to_string(&fa).unwrap();
    assert!(decoded.starts_with('>'), "should be FASTA format");
    assert!(!decoded.contains('+'), "should not have FASTQ quality");
    assert!(decoded.contains("ACGTACGT"), "sequence data preserved");
}

/// --no-mmap decode should produce identical output.
#[test]
fn no_mmap_decode_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");
    let fa_mmap = tmp.path().join("mmap.fa");
    let fa_nommap = tmp.path().join("nommap.fa");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode with mmap (default)
    let status = hecate()
        .args(["-d", "-w", "80"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa_mmap)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode with --no-mmap
    let status = hecate()
        .args(["-d", "--no-mmap", "-w", "80"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa_nommap)
        .status()
        .unwrap();
    assert!(status.success());

    let mmap_data = fs::read(&fa_mmap).unwrap();
    let nommap_data = fs::read(&fa_nommap).unwrap();
    assert_eq!(
        mmap_data, nommap_data,
        "mmap and no-mmap should produce identical output"
    );
}

/// --no-mmap slice should produce identical output.
#[test]
fn no_mmap_slice_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice with mmap (default)
    let output_mmap = hecate()
        .args(["-S", "/NC_000913/:1000001:2000000", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_mmap.status.success());

    // Slice with --no-mmap
    let output_nommap = hecate()
        .args([
            "--no-mmap",
            "-S",
            "/NC_000913/:1000001:2000000",
            "-c",
            "-w",
            "0",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_nommap.status.success());

    assert_eq!(
        output_mmap.stdout, output_nommap.stdout,
        "mmap and no-mmap slice should produce identical output"
    );
}

/// --filter as a no-op passthrough on encode should roundtrip.
#[test]
fn filter_encode_noop() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("filtered.he");
    let fa = tmp.path().join("decoded.fa");

    let status = hecate()
        .args(["-e", "-2", "--filter", "cat", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = normalize_fasta(&fs::read_to_string(data_path("small.fa")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "filter-encoded file should roundtrip");
}

/// --filter on decode should produce correct output.
#[test]
fn filter_decode_noop() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("encoded.he");
    let fa = tmp.path().join("decoded.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode through cat filter to a file
    let output = hecate()
        .args(["-d", "--filter", "cat", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "filter decode failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let original = normalize_fasta(&fs::read_to_string(data_path("small.fa")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "filter-decoded file should match");
}

/// --filter should be rejected in recode mode.
#[test]
fn filter_recode_rejected() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("encoded.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["recode", "--filter", "cat"])
        .arg(&he)
        .args(["-o"])
        .arg(tmp.path().join("out.he"))
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "filter should be rejected in recode mode"
    );
}

/// Indexed slice with --invert-case on E. coli (uses case checkpoints for
/// ranges past 1M bases).
#[test]
fn combo_indexed_slice_invert_case_ecoli() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice with invert-case at a range that benefits from checkpoint (past 2M)
    let output_inv = hecate()
        .args([
            "--invert-case",
            "-S",
            "/NC_000913/:2000001:2000100",
            "-c",
            "-w",
            "0",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_inv.status.success());

    // Normal slice (no invert)
    let output_normal = hecate()
        .args(["-S", "/NC_000913/:2000001:2000100", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_normal.status.success());

    let normal = String::from_utf8(output_normal.stdout).unwrap();
    let inverted = String::from_utf8(output_inv.stdout).unwrap();

    // The inverted sequence should have opposite case
    let normal_seq: &str = normal.lines().find(|l| !l.starts_with('>')).unwrap();
    let inv_seq: &str = inverted.lines().find(|l| !l.starts_with('>')).unwrap();
    assert_eq!(normal_seq.len(), inv_seq.len());
    for (n, i) in normal_seq.chars().zip(inv_seq.chars()) {
        if n.is_ascii_uppercase() {
            assert!(i.is_ascii_lowercase(), "inverted char should be lowercase");
        } else {
            assert!(i.is_ascii_uppercase(), "inverted char should be uppercase");
        }
    }
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: sequential (--no-mmap) paths
// ---------------------------------------------------------------------------

#[test]
fn verify_no_mmap_uncompressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["verify", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "verify --no-mmap failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(stderr.contains("OK"), "should report OK");
}

#[test]
fn verify_no_mmap_compressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["verify", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "verify --no-mmap failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn verify_no_mmap_nonindexed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["verify", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "verify --no-mmap non-indexed failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn stats_no_mmap() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["stats", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "stats --no-mmap failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert!(stdout.contains("Blocks"), "stats should mention blocks");
}

#[test]
fn stats_no_mmap_nonindexed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["stats", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "stats --no-mmap non-indexed failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn cat_no_mmap_indexed() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");
    let merged_fa = tmp.path().join("merged.fa");

    for out in [&a_he, &b_he] {
        let s = hecate()
            .args([
                "-e",
                "-2",
                "-I=full",
                "-M",
                "zstd:3",
                &data_path("small.fa"),
                "-o",
            ])
            .arg(out)
            .status()
            .unwrap();
        assert!(s.success());
    }

    // Force sequential path via --no-mmap
    let o = hecate()
        .args(["cat", "--no-mmap", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "cat --no-mmap failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&merged_he)
        .args(["-o"])
        .arg(&merged_fa)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&merged_fa).unwrap();
    let expected = format!("{}{}", original, original);
    assert_eq!(decoded, expected);
}

#[test]
fn cat_no_mmap_nonindexed() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");
    let merged_fa = tmp.path().join("merged.fa");

    for out in [&a_he, &b_he] {
        let s = hecate()
            .args(["-e", "-2", &data_path("small.fa"), "-o"])
            .arg(out)
            .status()
            .unwrap();
        assert!(s.success());
    }

    let o = hecate()
        .args(["cat", "--no-mmap", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "cat --no-mmap non-indexed failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&merged_he)
        .args(["-o"])
        .arg(&merged_fa)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&merged_fa).unwrap();
    let expected = format!("{}{}", original, original);
    assert_eq!(decoded, expected);
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: error paths
// ---------------------------------------------------------------------------

#[test]
fn error_cat_too_few_inputs() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let out_he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&a_he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .args(["-o"])
        .arg(&out_he)
        .output()
        .unwrap();
    assert!(!o.status.success(), "cat with 1 file should fail");
}

#[test]
fn error_cat_missing_output() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");

    for out in [&a_he, &b_he] {
        let s = hecate()
            .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
            .arg(out)
            .status()
            .unwrap();
        assert!(s.success());
    }

    let o = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .output()
        .unwrap();
    assert!(!o.status.success(), "cat without -o should fail");
}

#[test]
fn error_encode_joint_case_non_8bit() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // --joint-case requires 8-bit
    let o = hecate()
        .args(["-e", "-2", "--joint-case", &data_path("small.fa"), "-o"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(!o.status.success(), "joint-case with 2-bit should fail");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: verbose output
// ---------------------------------------------------------------------------

#[test]
fn verbose_encode_decode_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let o = hecate()
        .args([
            "-e",
            "-2",
            "-v",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        stderr.contains("encode"),
        "verbose encode should print timing"
    );

    let o = hecate()
        .args(["-d", "-w", "0", "-v"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .output()
        .unwrap();
    assert!(o.status.success());
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        stderr.contains("decode"),
        "verbose decode should print timing"
    );
}

#[test]
fn verbose_cat() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");

    for out in [&a_he, &b_he] {
        let s = hecate()
            .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
            .arg(out)
            .status()
            .unwrap();
        assert!(s.success());
    }

    let o = hecate()
        .args(["cat", "-v", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        stderr.contains("merged"),
        "verbose cat should print merge info"
    );
}
