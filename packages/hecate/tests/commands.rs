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

use common::{data_path, extract_bases, extract_bases_preserving_case, hecate, normalize_fasta};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// List command
// ---------------------------------------------------------------------------

#[test]
fn list_sequences() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 sequences");
    assert!(lines[0].starts_with("seq1 first sequence\t"));
    assert!(lines[0].contains("16"), "seq1 should have 16 bases");
    assert!(lines[1].starts_with("seq2 second sequence\t"));
    assert!(lines[1].contains("32"), "seq2 should have 32 bases");
}

// ---------------------------------------------------------------------------
// Recode command
// ---------------------------------------------------------------------------

#[test]
fn recode_changes_compression() {
    let tmp = TempDir::new().unwrap();
    let he1 = tmp.path().join("first.he");
    let he2 = tmp.path().join("second.he");
    let fa = tmp.path().join("out.fa");

    // Encode with zstd
    let status = hecate()
        .args(["-e", "-2", "-M", "zstd:3", &data_path("small.fa"), "-o"])
        .arg(&he1)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode with no compression
    let status = hecate()
        .arg("recode")
        .args(["-M", ""])
        .arg(&he1)
        .args(["-o"])
        .arg(&he2)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode recoded file
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he2)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "recode should preserve data");
}

// ---------------------------------------------------------------------------
// --strip-quality tests
// ---------------------------------------------------------------------------

#[test]
fn strip_quality_decode_fastq_to_fasta() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("reads.he");

    // Encode FASTQ
    let status = hecate()
        .args(["-e", "-2", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode with --strip-quality should produce FASTA output
    let output = hecate()
        .args(["--strip-quality", "-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "strip-quality decode failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    // Should have FASTA headers (>) not FASTQ headers (@)
    assert!(decoded.contains('>'), "output should be FASTA format");
    assert!(
        !decoded.contains('+'),
        "output should not contain FASTQ '+' lines"
    );
    // Verify all sequences are present
    let seq_count = decoded.lines().filter(|l| l.starts_with('>')).count();
    assert!(seq_count > 0, "should have at least one sequence");
}

#[test]
fn strip_quality_decode_marv_fastq() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("marv.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3",
            &data_path("SRR25496580-MARV.fastq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode with --strip-quality
    let output = hecate()
        .args(["--strip-quality", "-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(decoded.starts_with('>'), "should produce FASTA output");

    // Decode without --strip-quality for comparison
    let output_fq = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_fq.status.success());

    let decoded_fq = String::from_utf8(output_fq.stdout).unwrap();
    assert!(
        decoded_fq.starts_with('@'),
        "normal decode should produce FASTQ"
    );

    // Bases should match between FASTA and FASTQ outputs
    let fasta_bases = extract_bases(&decoded);
    let fastq_bases: String = decoded_fq
        .lines()
        .enumerate()
        .filter(|(i, _)| i % 4 == 1)
        .map(|(_, l)| l)
        .collect();
    assert_eq!(fasta_bases, fastq_bases, "bases should match");
}

#[test]
fn strip_quality_recode_fastq_to_fasta() {
    let tmp = TempDir::new().unwrap();
    let he_fq = tmp.path().join("reads.he");
    let he_fa = tmp.path().join("stripped.he");
    let fa = tmp.path().join("stripped.fa");

    // Encode FASTQ
    let status = hecate()
        .args(["-e", "-2", &data_path("small.fq"), "-o"])
        .arg(&he_fq)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode with --strip-quality
    let status = hecate()
        .args(["recode", "--strip-quality"])
        .arg(&he_fq)
        .args(["-o"])
        .arg(&he_fa)
        .status()
        .unwrap();
    assert!(status.success(), "recode strip-quality failed");

    // Decode the recoded file -- should be FASTA
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he_fa)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let decoded = fs::read_to_string(&fa).unwrap();
    assert!(
        decoded.starts_with('>'),
        "recoded file should decode as FASTA"
    );
    assert!(
        !decoded.contains('+'),
        "should not contain FASTQ quality lines"
    );
}

#[test]
fn strip_quality_slice_fastq() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("reads.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice with --strip-quality
    let output = hecate()
        .args(["--strip-quality", "-S", "/./", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "strip-quality slice failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    assert!(decoded.contains('>'), "sliced output should be FASTA");
    assert!(!decoded.contains('+'), "should not contain FASTQ quality");
}

// ---------------------------------------------------------------------------
// --invert-case tests
// ---------------------------------------------------------------------------

#[test]
fn invert_case_decode_fasta() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Normal decode
    let output_normal = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output_normal.status.success());
    let normal = String::from_utf8(output_normal.stdout).unwrap();

    // Decode with --invert-case
    let output_inverted = hecate()
        .args(["--invert-case", "-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output_inverted.status.success(),
        "invert-case decode failed: {}",
        String::from_utf8_lossy(&output_inverted.stderr)
    );
    let inverted = String::from_utf8(output_inverted.stdout).unwrap();

    // Headers should be the same
    let normal_headers: Vec<&str> = normal.lines().filter(|l| l.starts_with('>')).collect();
    let inverted_headers: Vec<&str> = inverted.lines().filter(|l| l.starts_with('>')).collect();
    assert_eq!(normal_headers, inverted_headers);

    // Bases should have swapped case
    let normal_bases = extract_bases_preserving_case(&normal);
    let inverted_bases = extract_bases_preserving_case(&inverted);
    assert_eq!(normal_bases.len(), inverted_bases.len());
    for (n, i) in normal_bases.chars().zip(inverted_bases.chars()) {
        if n.is_ascii_uppercase() {
            assert!(
                i.is_ascii_lowercase(),
                "uppercase '{}' should become lowercase, got '{}'",
                n,
                i
            );
        } else if n.is_ascii_lowercase() {
            assert!(
                i.is_ascii_uppercase(),
                "lowercase '{}' should become uppercase, got '{}'",
                n,
                i
            );
        }
    }
}

#[test]
fn invert_case_slice() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");

    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Slice with --invert-case
    let output = hecate()
        .args(["--invert-case", "-S", "/./", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "invert-case slice failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    let bases = extract_bases_preserving_case(&decoded);
    assert!(!bases.is_empty(), "should have output");

    // Compare with normal decode
    let output_normal = hecate()
        .args(["-S", "/./", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    let normal = String::from_utf8(output_normal.stdout).unwrap();
    let normal_bases = extract_bases_preserving_case(&normal);

    // Case should be inverted
    for (n, i) in normal_bases.chars().zip(bases.chars()) {
        if n.is_ascii_alphabetic() {
            assert_ne!(
                n.is_ascii_uppercase(),
                i.is_ascii_uppercase(),
                "case should be inverted for '{}'",
                n
            );
        }
    }
}

#[test]
fn invert_case_recode() {
    let tmp = TempDir::new().unwrap();
    let he_src = tmp.path().join("src.he");
    let he_inv = tmp.path().join("inverted.he");
    let fa = tmp.path().join("inverted.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("mixed_case.fa"), "-o"])
        .arg(&he_src)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode with --invert-case
    let status = hecate()
        .args(["recode", "--invert-case"])
        .arg(&he_src)
        .args(["-o"])
        .arg(&he_inv)
        .status()
        .unwrap();
    assert!(status.success(), "recode invert-case failed");

    // Decode the recoded file
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he_inv)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let decoded = fs::read_to_string(&fa).unwrap();
    let inverted_bases = extract_bases_preserving_case(&decoded);

    // Read original
    let original = fs::read_to_string(data_path("mixed_case.fa")).unwrap();
    let original_bases = extract_bases_preserving_case(&original);

    // Case should be inverted
    assert_eq!(original_bases.len(), inverted_bases.len());
    for (o, i) in original_bases.chars().zip(inverted_bases.chars()) {
        if o.is_ascii_alphabetic() {
            assert_ne!(
                o.is_ascii_uppercase(),
                i.is_ascii_uppercase(),
                "case should be inverted for '{}'",
                o
            );
        }
    }
}

#[test]
fn invert_case_double_invert_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("mixed.he");
    let he2 = tmp.path().join("inv1.he");
    let he3 = tmp.path().join("inv2.he");
    let fa = tmp.path().join("roundtrip.fa");

    // Encode
    let status = hecate()
        .args(["-e", "-2", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Recode with --invert-case twice
    let status = hecate()
        .args(["recode", "--invert-case"])
        .arg(&he)
        .args(["-o"])
        .arg(&he2)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["recode", "--invert-case"])
        .arg(&he2)
        .args(["-o"])
        .arg(&he3)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode - should match original
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he3)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = normalize_fasta(&fs::read_to_string(data_path("mixed_case.fa")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "double invert should be identity");
}

// ---------------------------------------------------------------------------
// Verify command
// ---------------------------------------------------------------------------

#[test]
fn verify_uncompressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["verify"]).arg(&he).output().unwrap();
    assert!(
        output.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("OK"), "verify should print OK");
}

#[test]
fn verify_compressed_indexed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
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
    assert!(status.success());

    let output = hecate().args(["verify"]).arg(&he).output().unwrap();
    assert!(
        output.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn verify_detects_corruption() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    // Corrupt a byte in the middle of the file
    let mut data = fs::read(&he).unwrap();
    let mid = data.len() / 2;
    data[mid] ^= 0xFF;
    fs::write(&he, &data).unwrap();

    let output = hecate().args(["verify"]).arg(&he).output().unwrap();
    assert!(
        !output.status.success(),
        "verify should fail on corrupted file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CRC mismatch") || stderr.contains("error"),
        "verify should report CRC error, got: {}",
        stderr
    );
}

// ---------------------------------------------------------------------------
// Stats command
// ---------------------------------------------------------------------------

#[test]
fn stats_basic() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(
        output.status.success(),
        "stats failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FASTA"), "should mention FASTA");
    assert!(stdout.contains("2-bit"), "should mention 2-bit");
    assert!(stdout.contains("NUC"), "should show NUC stream");
    assert!(stdout.contains("CTRL"), "should show CTRL stream");
}

#[test]
fn stats_compressed_shows_ratios() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

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

    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("zstd:3"), "should show compression method");
    assert!(stdout.contains("Sequences:"), "should show sequence count");
}

// ---------------------------------------------------------------------------
// Cat command
// ---------------------------------------------------------------------------

#[test]
fn cat_indexed_files() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");
    let merged_fa = tmp.path().join("merged.fa");

    // Encode two files with same settings
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&a_he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&b_he)
        .status()
        .unwrap();
    assert!(status.success());

    // Cat
    let output = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cat failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Decode merged
    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&merged_he)
        .args(["-o"])
        .arg(&merged_fa)
        .status()
        .unwrap();
    assert!(status.success());

    // Verify content = small.fa concatenated twice
    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&merged_fa).unwrap();
    let expected = format!("{}{}", original, original);
    assert_eq!(decoded, expected, "merged file should contain both copies");
}

#[test]
fn cat_mismatched_headers_fails() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let out_he = tmp.path().join("out.he");

    // Encode with different alphabets
    let status = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&a_he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-e", "-4", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&b_he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&out_he)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "cat should fail with mismatched headers"
    );
}

#[test]
fn cat_indexed_streaming_decode() {
    // Concatenated indexed files must be decodable via the streaming path
    // (i.e. piping through stdin). This verifies that intermediate TAG_EOF
    // markers are stripped during cat.
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");

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

    let o = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "cat failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    // Decode via stdin (forces streaming path, no seeking).
    let merged_data = fs::read(&merged_he).unwrap();
    let o = hecate()
        .args(["-d", "-w", "0"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    let mut child = o;
    child.stdin.take().unwrap().write_all(&merged_data).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "streaming decode failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = String::from_utf8(output.stdout).unwrap();
    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let expected = format!("{}{}", original, original);
    assert_eq!(
        decoded, expected,
        "streaming decode of concatenated indexed file should produce all sequences"
    );
}

#[test]
fn cat_nonindexed_raw_round_trip() {
    // Non-indexed raw files can be concatenated; TAG_EOF is stripped from
    // intermediate files' last blocks.
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
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "cat failed: {}",
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
    assert_eq!(
        decoded, expected,
        "non-indexed cat should produce all sequences"
    );
}

#[test]
fn cat_nonindexed_compressed_rejected() {
    // Non-indexed files with streaming compression cannot be concatenated.
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let out_he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", "-M", "zstd:3", &data_path("small.fa"), "-o"])
        .arg(&a_he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&a_he)
        .args(["-o"])
        .arg(&out_he)
        .output()
        .unwrap();
    assert!(
        !o.status.success(),
        "cat should reject non-indexed compressed files"
    );
    let stderr = String::from_utf8_lossy(&o.stderr);
    assert!(
        stderr.contains("cannot concatenate non-indexed compressed"),
        "error message should explain why: {}",
        stderr
    );
}

#[test]
fn cat_indexed_verify_after_merge() {
    // Merged indexed file must pass verification (CRC integrity).
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");

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

    let o = hecate()
        .args(["cat", "-i"])
        .arg(&a_he)
        .arg(&b_he)
        .args(["-o"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "cat failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let o = hecate().args(["verify"]).arg(&merged_he).output().unwrap();
    assert!(
        o.status.success(),
        "verify failed on merged file: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: recode
// ---------------------------------------------------------------------------

#[test]
fn recode_add_compression() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("raw.he");
    let recoded = tmp.path().join("compressed.he");
    let fa = tmp.path().join("out.fa");

    // Encode raw
    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Recode to add zstd
    let s = hecate()
        .args(["recode", "-M", "zstd:3", "-I=full"])
        .arg(&he)
        .args(["-o"])
        .arg(&recoded)
        .status()
        .unwrap();
    assert!(s.success(), "recode failed");

    // Decode and verify
    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&recoded)
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
fn recode_strip_compression() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("compressed.he");
    let recoded = tmp.path().join("raw.he");
    let fa = tmp.path().join("out.fa");

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

    // Recode without compression
    let s = hecate()
        .args(["recode", "-I=full"])
        .arg(&he)
        .args(["-o"])
        .arg(&recoded)
        .status()
        .unwrap();
    assert!(s.success(), "recode strip failed");

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&recoded)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(decoded, original);
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: per-stream compression pattern
// ---------------------------------------------------------------------------

#[test]
fn custom_pattern_selective_compression() {
    // Compress only NUC stream, leave others raw
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            "-P",
            "_0___",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "selective pattern encode failed");

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

// ---------------------------------------------------------------------------
// Coverage-driven tests: multi-block with E. coli (sequential paths)
// ---------------------------------------------------------------------------

#[test]
fn verify_no_mmap_ecoli_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
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
    assert!(s.success());

    let o = hecate()
        .args(["verify", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "verify --no-mmap E. coli failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn stats_no_mmap_ecoli_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
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
    assert!(s.success());

    let o = hecate()
        .args(["stats", "--no-mmap"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "stats --no-mmap E. coli failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let stdout = String::from_utf8_lossy(&o.stdout);
    assert!(stdout.contains("Blocks"), "should report block count");
    assert!(stdout.contains("Sequences"), "should report sequence count");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: cat with --no-mmap on multi-sequence file
// ---------------------------------------------------------------------------

#[test]
fn cat_no_mmap_multi_seq_indexed() {
    let tmp = TempDir::new().unwrap();
    let a_he = tmp.path().join("a.he");
    let b_he = tmp.path().join("b.he");
    let merged_he = tmp.path().join("merged.he");

    for (src, out) in [
        (&data_path("GCF_000005845.2.fna"), &a_he),
        (&data_path("GCF_000005845.2.fna"), &b_he),
    ] {
        let s = hecate()
            .args(["-e", "-2", "-I=full", "-M", "zstd:3", src, "-o"])
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
        "cat --no-mmap multi-seq failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    // Verify content
    let o = hecate()
        .args(["verify", "--no-mmap"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "verify merged failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    // Decode and check sequence count
    let o = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&merged_he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let decoded = String::from_utf8(o.stdout).unwrap();
    let seq_count = decoded.lines().filter(|l| l.starts_with('>')).count();
    assert_eq!(seq_count, 2, "merged file should have 2 sequences (1+1)");
}
