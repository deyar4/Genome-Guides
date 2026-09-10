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

mod common;

use common::{data_path, extract_bases, hecate, normalize_fasta};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Slice command
// ---------------------------------------------------------------------------

#[test]
fn slice_by_name() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "seq1 first sequence", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(
        decoded.contains(">seq1 first sequence"),
        "should contain seq1"
    );
    assert!(!decoded.contains(">seq2"), "should not contain seq2");
    assert!(decoded.contains("ACGTACGTACGTACGT"));
}

// ---------------------------------------------------------------------------
// Slice sub-range tests
// ---------------------------------------------------------------------------

/// Extract a 1-based inclusive sub-range from a base string.
fn subseq(bases: &str, start_1based: usize, end_1based: usize) -> &str {
    &bases[start_1based - 1..end_1based]
}

/// Encode a FASTA file with the given alphabet, then slice and decode.
/// Returns the decoded text.
fn encode_slice_decode(input_file: &str, alphabet: &str, slice_spec: &str) -> String {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", alphabet, input_file, "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate()
        .args(["-S", slice_spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "slice failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

const ECOLI_HEADER: &str = "NC_000913.3 Escherichia coli str. K-12 substr. MG1655, complete genome";

/// Helper: encode E. coli with given alphabet, slice a base range, return decoded bases.
fn ecoli_slice(alphabet: &str, start: usize, end: usize) -> String {
    let spec = format!("{}:{}:{}", ECOLI_HEADER, start, end);
    let decoded = encode_slice_decode(&data_path("GCF_000005845.2.fna"), alphabet, &spec);
    extract_bases(&decoded)
}

/// Load the E. coli reference bases (unwrapped) for oracle comparisons.
fn ecoli_reference() -> String {
    let fasta = fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap();
    extract_bases(&fasta)
}

// -- small.fa slice tests (quick, simple) --

#[test]
fn slice_small_fa_first_bases_2bit() {
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:1:4");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGT");
}

#[test]
fn slice_small_fa_mid_byte_2bit() {
    // Slice bases 3-6 (1-based) from seq1 = "GTAC" (mid-byte in 2-bit)
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:3:6");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "GTAC");
}

#[test]
fn slice_small_fa_single_base_2bit() {
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:1:1");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "A");
}

#[test]
fn slice_small_fa_last_base_2bit() {
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:16:16");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "T");
}

#[test]
fn slice_small_fa_entire_seq_2bit() {
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:1:16");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGTACGTACGTACGT");
}

#[test]
fn slice_small_fa_mid_byte_4bit() {
    // 4-bit: 2 bases/byte. Slicing at odd position = mid-byte.
    let decoded = encode_slice_decode(&data_path("small.fa"), "-4", "seq1 first sequence:2:5");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "CGTA");
}

#[test]
fn slice_small_fa_8bit() {
    let decoded = encode_slice_decode(&data_path("small.fa"), "-8", "seq1 first sequence:5:12");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGTACGT");
}

#[test]
fn slice_small_fa_second_seq() {
    // seq2 = AAAACCCCGGGGTTTTACGTACGTACGTACGT (32 bases)
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq2 second sequence:5:8");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "CCCC");
}

// -- small.fq slice tests (FASTQ with quality) --

#[test]
fn slice_small_fq_2bit() {
    // read1: ACGTACGT, quality: IIIIIIII
    let decoded = encode_slice_decode(&data_path("small.fq"), "-2", "read1 first read:3:6");
    // Should contain the sliced bases and quality
    assert!(decoded.contains("GTAC"), "bases: {}", decoded);
    assert!(decoded.contains("IIII"), "quality: {}", decoded);
}

// -- E. coli 2-bit slice tests --

#[test]
fn slice_ecoli_2bit_first_10() {
    let bases = ecoli_slice("-2", 1, 10);
    assert_eq!(bases, "AGCTTTTCAT");
}

#[test]
fn slice_ecoli_2bit_single_base() {
    let bases = ecoli_slice("-2", 1, 1);
    assert_eq!(bases, "A");
}

#[test]
fn slice_ecoli_2bit_mid_byte() {
    // Bases 3-6: mid-byte start and end (nuc_skip=2 in 2-bit)
    let bases = ecoli_slice("-2", 3, 6);
    assert_eq!(bases, "CTTT");
}

#[test]
fn slice_ecoli_2bit_simple_range() {
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 100, 200);
    let bases = ecoli_slice("-2", 100, 200);
    assert_eq!(bases, expected);
}

#[test]
fn slice_ecoli_2bit_block_boundary_straddle() {
    // 2-bit block boundary: 1 MiB = 1,048,576 bytes = 4,194,304 bases
    // Straddle: bases 4194303-4194306
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 4194303, 4194306);
    let bases = ecoli_slice("-2", 4194303, 4194306);
    assert_eq!(bases, expected, "2-bit block boundary straddle");
}

#[test]
fn slice_ecoli_2bit_block_boundary_start() {
    // Start exactly at block boundary
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 4194304, 4194307);
    let bases = ecoli_slice("-2", 4194304, 4194307);
    assert_eq!(bases, expected, "2-bit block boundary start");
}

#[test]
fn slice_ecoli_2bit_last_3_bases() {
    let bases = ecoli_slice("-2", 4641650, 4641652);
    assert_eq!(bases, "TTC");
}

#[test]
fn slice_ecoli_2bit_very_last_base() {
    let bases = ecoli_slice("-2", 4641652, 4641652);
    assert_eq!(bases, "C");
}

#[test]
fn slice_ecoli_2bit_mid_genome() {
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 1000001, 1000010);
    let bases = ecoli_slice("-2", 1000001, 1000010);
    assert_eq!(bases, expected);
}

#[test]
fn slice_ecoli_2bit_entire_genome() {
    let ref_bases = ecoli_reference();
    let bases = ecoli_slice("-2", 1, 4641652);
    assert_eq!(bases, ref_bases, "full genome slice should match");
}

#[test]
fn slice_ecoli_2bit_max_skip() {
    // nuc_skip = 3 (maximum for 2-bit): start at base 4 (4 % 4 = 0? no: 0-based start=3, 3 % 4 = 3)
    // 1-based 4 -> 0-based 3, nuc_skip = 3 % 4 = 3
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 4, 10);
    let bases = ecoli_slice("-2", 4, 10);
    assert_eq!(bases, expected, "max nuc_skip=3 in 2-bit");
}

#[test]
fn slice_ecoli_2bit_byte_aligned() {
    // Start at base 5 (0-based 4, 4 % 4 = 0) -- byte-aligned, nuc_skip=0
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 5, 12);
    let bases = ecoli_slice("-2", 5, 12);
    assert_eq!(bases, expected, "byte-aligned start in 2-bit");
}

#[test]
fn slice_ecoli_2bit_large_span() {
    // Span across block boundary: bases 4000000-4200000 (200K bases, crosses 4194304)
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 4000000, 4200000);
    let bases = ecoli_slice("-2", 4000000, 4200000);
    assert_eq!(bases, expected, "large span crossing block boundary");
}

// -- E. coli 4-bit slice tests --

#[test]
fn slice_ecoli_4bit_first_10() {
    let bases = ecoli_slice("-4", 1, 10);
    assert_eq!(bases, "AGCTTTTCAT");
}

#[test]
fn slice_ecoli_4bit_mid_byte() {
    // 4-bit: 2 bases/byte. Odd start = mid-byte (nuc_skip=1).
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 2, 7);
    let bases = ecoli_slice("-4", 2, 7);
    assert_eq!(bases, expected, "mid-byte start in 4-bit");
}

#[test]
fn slice_ecoli_4bit_block_boundary_straddle() {
    // 4-bit block boundary: 1 MiB = 1,048,576 bytes = 2,097,152 bases
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 2097151, 2097154);
    let bases = ecoli_slice("-4", 2097151, 2097154);
    assert_eq!(bases, expected, "4-bit block boundary straddle");
}

#[test]
fn slice_ecoli_4bit_block_boundary_start() {
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 2097152, 2097155);
    let bases = ecoli_slice("-4", 2097152, 2097155);
    assert_eq!(bases, expected, "4-bit block boundary start");
}

#[test]
fn slice_ecoli_4bit_last_base() {
    let bases = ecoli_slice("-4", 4641652, 4641652);
    assert_eq!(bases, "C");
}

#[test]
fn slice_ecoli_4bit_large_span() {
    // Cross 4-bit block boundary
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 2000000, 2200000);
    let bases = ecoli_slice("-4", 2000000, 2200000);
    assert_eq!(bases, expected, "large span crossing 4-bit block boundary");
}

// -- E. coli 2-bit + zstd slice tests (compressed blocks) --

#[test]
fn slice_ecoli_2bit_zstd_block_boundary_straddle() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

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

    let spec = format!("{}:4194303:4194306", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 4194303, 4194306);
    assert_eq!(bases, expected, "zstd: block boundary straddle");
}

#[test]
fn slice_ecoli_2bit_zstd_mid_byte() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

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

    let spec = format!("{}:3:6", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "CTTT", "zstd: mid-byte slice");
}

// -- Slice with line wrapping --

#[test]
fn slice_ecoli_2bit_line_wrap() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice 100 bases with line width 40
    let spec = format!("{}:100:199", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "40"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    let mut seq_lines = Vec::new();
    for line in decoded.lines() {
        if !line.starts_with('>') {
            assert!(line.len() <= 40, "line exceeds wrap width: {}", line.len());
            seq_lines.push(line.to_string());
        }
    }
    // 100 bases with width 40 = 3 lines (40+40+20)
    assert_eq!(seq_lines.len(), 3, "expected 3 wrapped lines");
    let joined: String = seq_lines.concat();
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 100, 199);
    assert_eq!(joined, expected, "wrapped content should match");
}

// -- Slice by name (no range) still works --

#[test]
fn slice_ecoli_by_name_whole_sequence() {
    let decoded = encode_slice_decode(&data_path("GCF_000005845.2.fna"), "-2", ECOLI_HEADER);
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    assert_eq!(bases, ref_bases, "name-only slice = full genome");
}

// ---------------------------------------------------------------------------
// Indexed slice tests
// ---------------------------------------------------------------------------

/// Helper: encode with indexing, then slice and decode.
fn indexed_encode_slice_decode(input_file: &str, alphabet: &str, slice_spec: &str) -> String {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", alphabet, "-I=full", input_file, "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "indexed encode failed");

    let output = hecate()
        .args(["-S", slice_spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "indexed slice failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn test_indexed_encode_decode_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "indexed encode failed");

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success(), "decode failed");

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "indexed round-trip mismatch");
}

#[test]
fn test_indexed_slice_by_name() {
    let decoded = indexed_encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGTACGTACGTACGT");
    assert!(decoded.contains(">seq1 first sequence"));
    assert!(!decoded.contains(">seq2"));
}

#[test]
fn test_indexed_slice_by_regex() {
    let decoded = indexed_encode_slice_decode(&data_path("small.fa"), "-2", "/seq2/");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "AAAACCCCGGGGTTTTACGTACGTACGTACGT");
    assert!(decoded.contains(">seq2 second sequence"));
    assert!(!decoded.contains(">seq1"));
}

#[test]
fn test_indexed_slice_with_range() {
    let decoded =
        indexed_encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence:1:4");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGT");
}

#[test]
fn test_indexed_slice_ecoli_2bit() {
    let decoded = indexed_encode_slice_decode(
        &data_path("GCF_000005845.2.fna"),
        "-2",
        &format!("{}:1000:2000", ECOLI_HEADER),
    );
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 1000, 2000);
    assert_eq!(bases, expected, "indexed 2-bit E. coli slice mismatch");
}

#[test]
fn test_indexed_slice_ecoli_4bit() {
    let decoded = indexed_encode_slice_decode(
        &data_path("GCF_000005845.2.fna"),
        "-4",
        &format!("{}:500:1500", ECOLI_HEADER),
    );
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 500, 1500);
    assert_eq!(bases, expected, "indexed 4-bit E. coli slice mismatch");
}

#[test]
fn test_indexed_slice_ecoli_full_genome() {
    let decoded =
        indexed_encode_slice_decode(&data_path("GCF_000005845.2.fna"), "-2", ECOLI_HEADER);
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    assert_eq!(bases, ref_bases, "indexed full genome slice mismatch");
}

#[test]
fn test_indexed_slice_ecoli_zstd() {
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
    assert!(status.success(), "indexed zstd encode failed");

    let spec = format!("{}:1000:2000", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "indexed zstd slice failed");

    let decoded = String::from_utf8(output.stdout).unwrap();
    let bases = extract_bases(&decoded);
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 1000, 2000);
    assert_eq!(bases, expected, "indexed zstd E. coli slice mismatch");
}

#[test]
fn test_indexed_fallback_to_sequential() {
    // Encode WITHOUT -I, slice should still work (sequential fallback)
    let decoded = encode_slice_decode(&data_path("small.fa"), "-2", "seq1 first sequence");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGTACGTACGTACGT");
}

#[test]
fn test_indexed_slice_mixed_case() {
    let decoded = indexed_encode_slice_decode(&data_path("mixed_case.fa"), "-2", "/gene1/");
    assert!(decoded.contains(">gene1"), "should contain gene1");
    let bases = extract_bases(&decoded);
    assert_eq!(bases, "ACGTacgtACGT", "mixed case should be preserved");
}

// ---------------------------------------------------------------------------
// Detailed system tests: list, skip_mask, indexed slice, mode precedence
// ---------------------------------------------------------------------------

// Headers contain colons (O157:H7) which conflict with the name:start:end
// slice spec syntax, so we use regex patterns for matching.
const ECOLI_SAKAI_SPEC: &str = "/^NC_002695\\.2/";
const ECOLI_POSAK1_SPEC: &str = "/^NC_002127\\.1/";
const ECOLI_PO157_SPEC: &str = "/^NC_002128\\.1/";

/// Load reference bases for E. coli O157:H7 Sakai main chromosome.
fn sakai_reference() -> String {
    let fasta = fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap();
    // Extract bases only for the first sequence (main chromosome).
    let mut bases = String::new();
    let mut in_first = false;
    for line in fasta.lines() {
        if line.starts_with('>') {
            if in_first {
                break;
            }
            if line[1..].starts_with("NC_002695.2") {
                in_first = true;
            }
        } else if in_first {
            bases.push_str(line);
        }
    }
    bases
}

/// Extract bases for a given header from a multi-sequence FASTA string.
fn extract_bases_for(fasta: &str, header_prefix: &str) -> String {
    let mut bases = String::new();
    let mut capturing = false;
    for line in fasta.lines() {
        if line.starts_with('>') {
            if capturing {
                break;
            }
            if line[1..].starts_with(header_prefix) {
                capturing = true;
            }
        } else if capturing {
            bases.push_str(line);
        }
    }
    bases
}

// ---------------------------------------------------------------------------
// List tests on E. coli genomes (exercises skip_mask / needs_stream_data)
// ---------------------------------------------------------------------------

#[test]
fn list_ecoli_k12_uncompressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "K-12 has 1 sequence");
    assert!(lines[0].starts_with("NC_000913.3"));
    assert!(
        lines[0].ends_with("4641652"),
        "expected 4641652 bases, got: {}",
        lines[0]
    );
}

#[test]
fn list_ecoli_k12_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

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

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("4641652"));
}

#[test]
fn list_ecoli_k12_indexed_zstd() {
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

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("4641652"));
}

#[test]
fn list_ecoli_sakai_multi_seq_uncompressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("GCF_000008865.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "Sakai has 3 sequences (chromosome + 2 plasmids)"
    );
    assert!(lines[0].contains("NC_002695.2") && lines[0].ends_with("5498578"));
    assert!(lines[1].contains("NC_002127.1") && lines[1].ends_with("3306"));
    assert!(lines[2].contains("NC_002128.1") && lines[2].ends_with("92721"));
}

#[test]
fn list_ecoli_sakai_multi_seq_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("5498578"));
    assert!(lines[1].ends_with("3306"));
    assert!(lines[2].ends_with("92721"));
}

#[test]
fn list_ecoli_sakai_indexed_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");

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

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("5498578"));
    assert!(lines[1].ends_with("3306"));
    assert!(lines[2].ends_with("92721"));
}

#[test]
fn list_combined_ecoli_genomes_zstd() {
    let tmp = TempDir::new().unwrap();
    let combined_fa = tmp.path().join("combined.fa");
    let he = tmp.path().join("combined.he");

    // Concatenate both E. coli genomes
    let fa1 = fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap();
    let fa2 = fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap();
    fs::write(&combined_fa, format!("{}{}", fa1, fa2)).unwrap();

    let status = hecate()
        .args(["-e", "-2", "-M", "zstd:3"])
        .arg(&combined_fa)
        .args(["-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "should list 4 sequences (1 K-12 + 3 Sakai)");
    assert!(lines[0].contains("NC_000913.3") && lines[0].ends_with("4641652"));
    assert!(lines[1].contains("NC_002695.2") && lines[1].ends_with("5498578"));
    assert!(lines[2].contains("NC_002127.1") && lines[2].ends_with("3306"));
    assert!(lines[3].contains("NC_002128.1") && lines[3].ends_with("92721"));
}

// ---------------------------------------------------------------------------
// Indexed slice tests on multi-sequence E. coli Sakai genome
// ---------------------------------------------------------------------------

/// Helper: encode Sakai with indexing + zstd, then slice and decode.
fn sakai_indexed_slice(slice_spec: &str) -> String {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");

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
    assert!(status.success(), "indexed encode failed");

    let output = hecate()
        .args(["-S", slice_spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "indexed slice failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn indexed_slice_sakai_main_chromosome_by_name() {
    let decoded = sakai_indexed_slice(ECOLI_SAKAI_SPEC);
    let bases = extract_bases(&decoded);
    let ref_bases = sakai_reference();
    assert_eq!(bases.len(), ref_bases.len());
    assert_eq!(
        bases, ref_bases,
        "full chromosome slice should match reference"
    );
}

#[test]
fn indexed_slice_sakai_main_chromosome_by_regex() {
    let decoded = sakai_indexed_slice("/NC_002695/");
    assert!(decoded.contains(">NC_002695.2"));
    assert!(!decoded.contains(">NC_002127.1"));
    assert!(!decoded.contains(">NC_002128.1"));
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 5498578);
}

#[test]
fn indexed_slice_sakai_small_plasmid() {
    let decoded = sakai_indexed_slice(ECOLI_POSAK1_SPEC);
    assert!(decoded.contains(">NC_002127.1"));
    assert!(!decoded.contains(">NC_002695.2"));
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 3306, "pOSAK1 plasmid should be 3306 bases");
}

#[test]
fn indexed_slice_sakai_large_plasmid() {
    let decoded = sakai_indexed_slice(ECOLI_PO157_SPEC);
    assert!(decoded.contains(">NC_002128.1"));
    assert!(!decoded.contains(">NC_002695.2"));
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 92721, "pO157 plasmid should be 92721 bases");
}

#[test]
fn indexed_slice_sakai_plasmid_by_regex() {
    // Match both plasmids via regex
    let decoded = sakai_indexed_slice("/plasmid/");
    assert!(
        !decoded.contains(">NC_002695.2"),
        "chromosome should be excluded"
    );
    assert!(
        decoded.contains(">NC_002127.1"),
        "pOSAK1 should be included"
    );
    assert!(decoded.contains(">NC_002128.1"), "pO157 should be included");

    let posak1_bases = extract_bases_for(&decoded, "NC_002127.1");
    let po157_bases = extract_bases_for(&decoded, "NC_002128.1");
    assert_eq!(posak1_bases.len(), 3306);
    assert_eq!(po157_bases.len(), 92721);
}

#[test]
fn indexed_slice_sakai_chromosome_subrange() {
    let decoded = sakai_indexed_slice("/^NC_002695\\.2/:1000:2000");
    let bases = extract_bases(&decoded);
    let ref_bases = sakai_reference();
    let expected = subseq(&ref_bases, 1000, 2000);
    assert_eq!(bases, expected, "indexed chromosome subrange mismatch");
}

#[test]
fn indexed_slice_sakai_chromosome_first_10() {
    let decoded = sakai_indexed_slice("/^NC_002695\\.2/:1:10");
    let bases = extract_bases(&decoded);
    let ref_bases = sakai_reference();
    let expected = subseq(&ref_bases, 1, 10);
    assert_eq!(bases, expected);
}

#[test]
fn indexed_slice_sakai_chromosome_last_10() {
    let decoded = sakai_indexed_slice("/^NC_002695\\.2/:5498569:5498578");
    let bases = extract_bases(&decoded);
    let ref_bases = sakai_reference();
    let expected = subseq(&ref_bases, 5498569, 5498578);
    assert_eq!(bases, expected);
}

#[test]
fn indexed_slice_sakai_chromosome_block_boundary() {
    // 2-bit block boundary at 4,194,304 bases (1 MiB * 4 bases/byte)
    let decoded = sakai_indexed_slice("/^NC_002695\\.2/:4194303:4194306");
    let bases = extract_bases(&decoded);
    let ref_bases = sakai_reference();
    let expected = subseq(&ref_bases, 4194303, 4194306);
    assert_eq!(
        bases, expected,
        "block boundary straddle on Sakai chromosome"
    );
}

#[test]
fn indexed_slice_sakai_small_plasmid_subrange() {
    // Slice bases 100-200 from the small plasmid (3306 bases total)
    let decoded = sakai_indexed_slice("/^NC_002127\\.1/:100:200");
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 101);

    // Verify against the raw FASTA
    let fasta = fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap();
    let plasmid_bases = extract_bases_for(&fasta, "NC_002127.1");
    let expected = subseq(&plasmid_bases, 100, 200);
    assert_eq!(bases, expected, "plasmid subrange mismatch");
}

#[test]
fn indexed_slice_sakai_nonexistent_name_empty() {
    // Slicing a name that doesn't exist should produce empty output
    let decoded = sakai_indexed_slice("nonexistent_sequence");
    assert!(
        decoded.is_empty() || decoded.trim().is_empty(),
        "non-matching slice should produce empty output"
    );
}

// ---------------------------------------------------------------------------
// Mode precedence: -S with -d should route to Slice, not Decode
// ---------------------------------------------------------------------------

#[test]
fn mode_precedence_dash_s_with_dash_d() {
    // Encode with full index
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // -S with -d should extract only the matching sequence, not the whole file
    let output = hecate()
        .args(["-S", ECOLI_POSAK1_SPEC, "-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mode precedence failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    // Should contain only the plasmid, not all 3 sequences
    assert!(decoded.contains(">NC_002127.1"), "should contain pOSAK1");
    assert!(
        !decoded.contains(">NC_002695.2"),
        "should NOT contain main chromosome"
    );
    assert!(
        !decoded.contains(">NC_002128.1"),
        "should NOT contain pO157"
    );
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 3306);
}

#[test]
fn mode_precedence_dash_s_regex_with_dash_d() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // -d -S /regex/ should go through Slice mode
    let output = hecate()
        .args(["-d", "-S", "/pO157/", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(decoded.contains(">NC_002128.1"), "should contain pO157");
    assert!(
        !decoded.contains(">NC_002695.2"),
        "should NOT contain chromosome"
    );
    let bases = extract_bases(&decoded);
    assert_eq!(bases.len(), 92721);
}

// ---------------------------------------------------------------------------
// Indexed vs sequential slice consistency
// ---------------------------------------------------------------------------

#[test]
fn indexed_vs_sequential_slice_consistency_sakai() {
    let tmp = TempDir::new().unwrap();
    let he_indexed = tmp.path().join("indexed.he");
    let he_plain = tmp.path().join("plain.he");

    // Encode with full index
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
        .arg(&he_indexed)
        .status()
        .unwrap();
    assert!(status.success());

    // Encode without index (sequential fallback)
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he_plain)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice the same range from both
    let spec = "/^NC_002695\\.2/:5000:10000".to_string();

    let out_indexed = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he_indexed)
        .output()
        .unwrap();
    assert!(out_indexed.status.success());

    let out_plain = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he_plain)
        .output()
        .unwrap();
    assert!(out_plain.status.success());

    let decoded_indexed = String::from_utf8(out_indexed.stdout).unwrap();
    let decoded_plain = String::from_utf8(out_plain.stdout).unwrap();
    assert_eq!(
        decoded_indexed, decoded_plain,
        "indexed and sequential slice should produce identical output"
    );
}

#[test]
fn indexed_vs_sequential_slice_consistency_plasmid() {
    let tmp = TempDir::new().unwrap();
    let he_indexed = tmp.path().join("indexed.he");
    let he_plain = tmp.path().join("plain.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            &data_path("GCF_000008865.2.fna"),
            "-o",
        ])
        .arg(&he_indexed)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-e", "-2", &data_path("GCF_000008865.2.fna"), "-o"])
        .arg(&he_plain)
        .status()
        .unwrap();
    assert!(status.success());

    // Extract the small plasmid entirely from both
    let out_indexed = hecate()
        .args(["-S", ECOLI_POSAK1_SPEC, "-c", "-w", "0"])
        .arg(&he_indexed)
        .output()
        .unwrap();
    assert!(out_indexed.status.success());

    let out_plain = hecate()
        .args(["-S", ECOLI_POSAK1_SPEC, "-c", "-w", "0"])
        .arg(&he_plain)
        .output()
        .unwrap();
    assert!(out_plain.status.success());

    assert_eq!(
        String::from_utf8(out_indexed.stdout).unwrap(),
        String::from_utf8(out_plain.stdout).unwrap(),
        "indexed and sequential plasmid slice should match"
    );
}

// ---------------------------------------------------------------------------
// Indexed slice with custom block budget (-I full,N)
// ---------------------------------------------------------------------------

#[test]
fn indexed_slice_custom_budget_small_blocks() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    // Encode with 1 MiB block budget (more, smaller blocks)
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full,1",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // List should still work
    let list_out = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(list_out.status.success());
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert!(stdout.contains("4641652"));

    // Slice should produce correct data
    let spec = format!("{}:100:200", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let bases = extract_bases(&String::from_utf8(output.stdout).unwrap());
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 100, 200);
    assert_eq!(bases, expected, "small-block indexed slice should match");
}

#[test]
fn indexed_slice_custom_budget_large_blocks() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    // Encode with 16 MiB block budget (fewer, larger blocks)
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full,16",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let list_out = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(list_out.status.success());
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert!(stdout.contains("4641652"));

    // Full genome round-trip via indexed slice
    let spec = ECOLI_HEADER;
    let output = hecate()
        .args(["-S", spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let bases = extract_bases(&String::from_utf8(output.stdout).unwrap());
    let ref_bases = ecoli_reference();
    assert_eq!(
        bases, ref_bases,
        "large-block full genome indexed slice mismatch"
    );
}

// ---------------------------------------------------------------------------
// List + full decode round-trip consistency
// ---------------------------------------------------------------------------

#[test]
fn list_then_decode_roundtrip_sakai_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("sakai.he");
    let fa = tmp.path().join("sakai.fa");

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

    // List first (exercises skip_mask path)
    let list_out = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(list_out.status.success());

    // Then decode fully (exercises normal path -- file must not be corrupted
    // by listing, which is a separate process so this just verifies the file)
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "full decode after list should match");
}

// ---------------------------------------------------------------------------
// Indexed slice on K-12 with different alphabets
// ---------------------------------------------------------------------------

#[test]
fn indexed_slice_ecoli_k12_4bit_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args([
            "-e",
            "-4",
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

    // List
    let list_out = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(list_out.status.success());
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert!(stdout.contains("4641652"));

    // Slice a range
    let spec = format!("{}:2097150:2097155", ECOLI_HEADER);
    let output = hecate()
        .args(["-S", &spec, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let bases = extract_bases(&String::from_utf8(output.stdout).unwrap());
    let ref_bases = ecoli_reference();
    let expected = subseq(&ref_bases, 2097150, 2097155);
    assert_eq!(
        bases, expected,
        "4-bit indexed slice block boundary mismatch"
    );
}

// ---------------------------------------------------------------------------
// Indexed slice across block boundaries on combined multi-genome file
// ---------------------------------------------------------------------------

#[test]
fn indexed_slice_combined_genomes_select_each() {
    let tmp = TempDir::new().unwrap();
    let combined_fa = tmp.path().join("combined.fa");
    let he = tmp.path().join("combined.he");

    let fa1 = fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap();
    let fa2 = fs::read_to_string(data_path("GCF_000008865.2.fna")).unwrap();
    fs::write(&combined_fa, format!("{}{}", fa1, fa2)).unwrap();

    let status = hecate()
        .args(["-e", "-2", "-I=full", "-M", "zstd:3"])
        .arg(&combined_fa)
        .args(["-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // List should show all 4 sequences
    let list_out = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(list_out.status.success());
    let stdout = String::from_utf8(list_out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 4);

    // Extract K-12 first 10 bases
    let spec_k12 = format!("{}:1:10", ECOLI_HEADER);
    let out = hecate()
        .args(["-S", &spec_k12, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(out.status.success());
    let bases = extract_bases(&String::from_utf8(out.stdout).unwrap());
    assert_eq!(bases, "AGCTTTTCAT");

    // Extract Sakai last 10 bases
    let spec_sakai = "/^NC_002695\\.2/:5498569:5498578";
    let out = hecate()
        .args(["-S", spec_sakai, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(out.status.success());
    let sakai_bases = extract_bases(&String::from_utf8(out.stdout).unwrap());
    let ref_sakai = sakai_reference();
    let expected = subseq(&ref_sakai, 5498569, 5498578);
    assert_eq!(sakai_bases, expected);

    // Extract small plasmid entirely
    let out = hecate()
        .args(["-S", ECOLI_POSAK1_SPEC, "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(out.status.success());
    let plasmid_bases = extract_bases(&String::from_utf8(out.stdout).unwrap());
    assert_eq!(plasmid_bases.len(), 3306);
}

// ---------------------------------------------------------------------------
// Numeric random access (#N syntax)
// ---------------------------------------------------------------------------

#[test]
fn ordinal_access_first_sequence() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "#0", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(
        decoded.contains("seq1 first sequence"),
        "should have first seq header"
    );
    assert!(
        decoded.contains("ACGTACGTACGTACGT"),
        "should have first seq data"
    );
    assert!(!decoded.contains("seq2"), "should not have second seq");
}

#[test]
fn ordinal_access_second_sequence() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "#1", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(!decoded.contains("seq1"), "should not have first seq");
    assert!(
        decoded.contains("seq2 second sequence"),
        "should have second seq header"
    );
    assert!(
        decoded.contains("AAAACCCCGGGGTTTTACGTACGTACGTACGT"),
        "should have second seq data"
    );
}

#[test]
fn ordinal_range_access() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "#0-2", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(decoded.contains("seq1"), "should have first seq");
    assert!(decoded.contains("seq2"), "should have second seq");
}

#[test]
fn ordinal_with_base_range() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "#0:1:10", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());
    let decoded = String::from_utf8(output.stdout).unwrap();
    let seq_line = decoded.lines().find(|l| !l.starts_with('>')).unwrap();
    assert_eq!(seq_line.len(), 10, "should have exactly 10 bases");
    assert_eq!(seq_line, "ACGTACGTAC", "first 10 bases of seq1");
}

#[test]
fn ordinal_sequential_path() {
    // Test ordinal access via the sequential (non-indexed) path
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // Encode WITHOUT indexing
    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["-S", "#1", "-c", "-w", "0", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(!decoded.contains("seq1"), "should not have first seq");
    assert!(decoded.contains("seq2"), "should have second seq");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: streaming (non-indexed) slice
// ---------------------------------------------------------------------------

#[test]
fn slice_nonindexed_by_name() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // Encode without index
    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Slice should work via streaming path (use regex for partial match)
    let o = hecate()
        .args(["-S", "/seq2/", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "non-indexed slice failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let decoded = String::from_utf8(o.stdout).unwrap();
    assert!(decoded.contains("seq2"), "should contain seq2");
    assert!(!decoded.contains("seq1"), "should not contain seq1");
}

#[test]
fn slice_nonindexed_by_regex() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-S", "/seq/", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let decoded = String::from_utf8(o.stdout).unwrap();
    assert!(decoded.contains("seq1"), "should have seq1");
    assert!(decoded.contains("seq2"), "should have seq2");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: block-only index tier
// ---------------------------------------------------------------------------

#[test]
fn indexed_block_tier_decode_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=block",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "block-tier encode failed");

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn indexed_block_tier_verify() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=block",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate().args(["verify"]).arg(&he).output().unwrap();
    assert!(
        o.status.success(),
        "verify block-tier failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
