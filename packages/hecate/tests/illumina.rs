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

use common::{data_path, hecate};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Basic Illumina FASTQ mode tests
// ---------------------------------------------------------------------------

#[test]
fn illumina_round_trip_2bit() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "illumina decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "illumina round-trip mismatch");
}

#[test]
fn illumina_round_trip_4bit() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-4", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina 4-bit encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "illumina 4-bit decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "illumina 4-bit round-trip mismatch");
}

#[test]
fn illumina_quality_binning() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-B",
            "illumina",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina+binning encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "illumina+binning decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();

    let orig_records: Vec<&str> = original.lines().collect();
    let dec_records: Vec<&str> = decoded.lines().collect();
    assert_eq!(
        orig_records.len(),
        dec_records.len(),
        "record count mismatch"
    );

    for i in (0..orig_records.len()).step_by(4) {
        assert_eq!(
            orig_records[i],
            dec_records[i],
            "header mismatch at record {}",
            i / 4
        );
        assert_eq!(
            orig_records[i + 1],
            dec_records[i + 1],
            "sequence mismatch at record {}",
            i / 4
        );
        assert_eq!(
            orig_records[i + 2],
            dec_records[i + 2],
            "plus line mismatch at record {}",
            i / 4
        );
        assert_eq!(
            orig_records[i + 3].len(),
            dec_records[i + 3].len(),
            "quality length mismatch at record {}",
            i / 4
        );
    }
}

#[test]
fn illumina_verify() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina encode failed");

    let status = hecate().args(["--verify"]).arg(&he).status().unwrap();
    assert!(status.success(), "illumina verify failed");
}

#[test]
fn illumina_list() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina encode failed");

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "illumina list failed");

    let listing = String::from_utf8(output.stdout).unwrap();
    assert_eq!(listing.lines().count(), 4, "expected 4 records in listing");
    assert!(listing.starts_with("A00488:61:HMLGNDSXX:4:1101:2692:1000 1:N:0:GATCAG"));
}

#[test]
fn illumina_stats() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina encode failed");

    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "illumina stats failed");

    let stats = String::from_utf8(output.stdout).unwrap();
    assert!(
        stats.contains("HDR_COORDS"),
        "stats should show HDR_COORDS stream"
    );
}

#[test]
fn illumina_cat_two_files() {
    let tmp = TempDir::new().unwrap();
    let he1 = tmp.path().join("a.he");
    let he2 = tmp.path().join("b.he");
    let cat_he = tmp.path().join("cat.he");
    let fq = tmp.path().join("out.fq");

    // Encode same file twice (no compression for non-indexed cat)
    let status = hecate()
        .args(["-e", "-2", "-L", "-M", "", &data_path("illumina.fq"), "-o"])
        .arg(&he1)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-e", "-2", "-L", "-M", "", &data_path("illumina.fq"), "-o"])
        .arg(&he2)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["cat", "-i"])
        .arg(&he1)
        .arg(&he2)
        .args(["-o"])
        .arg(&cat_he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina cat failed");

    let status = hecate()
        .args(["-d"])
        .arg(&cat_he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "illumina cat decode failed");

    let decoded = fs::read_to_string(&fq).unwrap();
    let record_count = decoded.lines().filter(|l| l.starts_with('@')).count();
    assert_eq!(record_count, 8, "expected 8 records after cat");
}

#[test]
fn illumina_with_compression() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-M",
            "zstd:3",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "illumina+zstd encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "illumina+zstd decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "illumina+zstd round-trip mismatch");
}

#[test]
fn illumina_rejects_fasta() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-A", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(!status.success(), "illumina+fasta should fail");
}

// ---------------------------------------------------------------------------
// Indexed Illumina tests
// ---------------------------------------------------------------------------

#[test]
fn illumina_indexed_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "indexed illumina encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "indexed illumina decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "indexed illumina round-trip mismatch");
}

#[test]
fn illumina_indexed_with_zstd_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "indexed illumina+zstd encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "indexed illumina+zstd decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(
        original, decoded,
        "indexed illumina+zstd round-trip mismatch"
    );
}

#[test]
fn illumina_indexed_verify() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let status = hecate().args(["--verify"]).arg(&he).status().unwrap();
    assert!(status.success(), "indexed illumina verify failed");
}

#[test]
fn illumina_indexed_list() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "indexed illumina list failed");

    let listing = String::from_utf8(output.stdout).unwrap();
    assert_eq!(listing.lines().count(), 4, "expected 4 records");
    assert!(listing.contains("A00488:61:HMLGNDSXX:4:2301:987:45678 1:Y:0:ACGT+TGCA"));
}

#[test]
fn illumina_indexed_stats() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "indexed illumina stats failed");

    let stats = String::from_utf8(output.stdout).unwrap();
    assert!(
        stats.contains("HDR_COORDS"),
        "should show HDR_COORDS stream"
    );
    assert!(
        stats.contains("Indexed: yes"),
        "should indicate indexed file"
    );
}

#[test]
fn illumina_indexed_slice_by_name() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    // Slice the third record by regex (use regex to avoid colon ambiguity)
    let output = hecate()
        .args(["-S", "/1201:15432:2890/", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "indexed illumina slice failed");

    let sliced = String::from_utf8(output.stdout).unwrap();
    assert!(
        sliced.contains("ACGTACGTACGTACGTACGTACGTACGTACGT"),
        "sliced output should contain the third record's sequence"
    );
}

#[test]
fn illumina_indexed_slice_by_ordinal() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    // Slice the last record (#3, 0-based)
    let output = hecate()
        .args(["-S", "#3", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "ordinal slice failed");

    let sliced = String::from_utf8(output.stdout).unwrap();
    assert!(
        sliced.contains("GGGGCCCCTTTTAAAA"),
        "sliced output should contain the fourth record's sequence"
    );
}

#[test]
fn illumina_recode_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("orig.he");
    let recode_he = tmp.path().join("recode.he");
    let fq = tmp.path().join("out.fq");

    // Encode without compression
    let status = hecate()
        .args(["-e", "-2", "-L", "-M", "", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    // Recode with zstd
    let status = hecate()
        .args(["recode", "-M", "zstd:3"])
        .arg(&he)
        .args(["-o"])
        .arg(&recode_he)
        .status()
        .unwrap();
    assert!(status.success(), "recode failed");

    // Decode recoded file
    let status = hecate()
        .args(["-d"])
        .arg(&recode_he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "recode decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "recode round-trip mismatch");
}

#[test]
fn illumina_strip_quality_to_fasta() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    // Decode with --strip-quality should produce FASTA
    let output = hecate()
        .args(["-d", "--strip-quality", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "strip-quality decode failed");

    let decoded = String::from_utf8(output.stdout).unwrap();
    // FASTA headers start with '>'
    assert!(decoded.starts_with('>'), "output should be FASTA");
    // Should have 4 sequences
    let seq_count = decoded.lines().filter(|l| l.starts_with('>')).count();
    assert_eq!(seq_count, 4, "expected 4 sequences");
    // Check sequences are preserved
    assert!(decoded.contains("ACGTACGTACGTACGT"));
    assert!(decoded.contains("TTTTCCCCAAAAGGG"));
    assert!(decoded.contains("GGGGCCCCTTTTAAAA"));
}

#[test]
fn illumina_quality_binning_uniform() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    // Test uniform binning with 4 bins
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-B",
            "bin:4",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "uniform binning encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "uniform binning decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();

    // Headers and sequences should match; quality is lossy
    let orig_lines: Vec<&str> = original.lines().collect();
    let dec_lines: Vec<&str> = decoded.lines().collect();
    assert_eq!(orig_lines.len(), dec_lines.len());
    for i in (0..orig_lines.len()).step_by(4) {
        assert_eq!(orig_lines[i], dec_lines[i]);
        assert_eq!(orig_lines[i + 1], dec_lines[i + 1]);
    }
}

#[test]
fn illumina_no_mmap_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "--no-mmap",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "no-mmap encode failed");

    let status = hecate()
        .args(["-d", "--no-mmap"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "no-mmap decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "no-mmap round-trip mismatch");
}

// ---------------------------------------------------------------------------
// Comprehensive pipeline test: indexed encode -> cat -> verify -> slice
// ---------------------------------------------------------------------------

#[test]
fn illumina_indexed_cat_verify_slice_pipeline() {
    let tmp = TempDir::new().unwrap();

    // Step 1: Encode illumina fixture (no compression, no index -- cat needs matching headers)
    let he1 = tmp.path().join("illumina.he");
    let status = hecate()
        .args(["-e", "-2", "-L", "-M", "", &data_path("illumina.fq"), "-o"])
        .arg(&he1)
        .status()
        .unwrap();
    assert!(status.success(), "step 1: illumina encode failed");

    // Step 2: Create a second .he file with a small extra FASTQ record
    let extra_fq = tmp.path().join("extra.fq");
    fs::write(
        &extra_fq,
        "\
@A00488:61:HMLGNDSXX:4:3101:5555:9999 1:N:0:AAAAAA
CCCCGGGGTTTTAAAA
+
IIIIIIIIIIIIIIII
",
    )
    .unwrap();

    let he2 = tmp.path().join("extra.he");
    let status = hecate()
        .args(["-e", "-2", "-L", "-M", ""])
        .arg(&extra_fq)
        .args(["-o"])
        .arg(&he2)
        .status()
        .unwrap();
    assert!(status.success(), "step 2: encode extra record failed");

    // Step 3: Cat the two files together
    let cat_he = tmp.path().join("cat.he");
    let status = hecate()
        .args(["cat", "-i"])
        .arg(&he1)
        .arg(&he2)
        .args(["-o"])
        .arg(&cat_he)
        .status()
        .unwrap();
    assert!(status.success(), "step 3: cat failed");

    // Step 4: Verify the concatenated file
    let status = hecate().args(["--verify"]).arg(&cat_he).status().unwrap();
    assert!(status.success(), "step 4: verify after cat failed");

    // Step 5: Decode the full file to check record count
    let full_fq = tmp.path().join("full.fq");
    let status = hecate()
        .args(["-d"])
        .arg(&cat_he)
        .args(["-o"])
        .arg(&full_fq)
        .status()
        .unwrap();
    assert!(status.success(), "step 5: full decode failed");

    let full_decoded = fs::read_to_string(&full_fq).unwrap();
    let record_count = full_decoded.lines().filter(|l| l.starts_with('@')).count();
    assert_eq!(record_count, 5, "expected 5 records after cat (4 + 1)");

    // Step 6: Slice the last record (#4, 0-based) with --strip-quality to get FASTA
    let output = hecate()
        .args(["-S", "#4", "--strip-quality", "-c", "-w", "0"])
        .arg(&cat_he)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "step 6: slice last record as FASTA failed"
    );

    let sliced = String::from_utf8(output.stdout).unwrap();
    // Should be a FASTA record for the extra sequence
    assert!(
        sliced.starts_with('>'),
        "output should be FASTA (starts with >)"
    );
    assert!(
        sliced.contains("A00488:61:HMLGNDSXX:4:3101:5555:9999"),
        "should contain the extra record's header"
    );
    assert!(
        sliced.contains("CCCCGGGGTTTTAAAA"),
        "should contain the extra record's sequence"
    );
    // Should NOT contain quality lines
    assert!(
        !sliced.contains("IIII"),
        "FASTA output should not contain quality"
    );
}

// ---------------------------------------------------------------------------
// Indexed cat with compression
// ---------------------------------------------------------------------------

#[test]
fn illumina_indexed_cat_compressed() {
    let tmp = TempDir::new().unwrap();
    let he1 = tmp.path().join("a.he");
    let he2 = tmp.path().join("b.he");
    let cat_he = tmp.path().join("cat.he");
    let fq = tmp.path().join("out.fq");

    // Encode two indexed illumina files with zstd compression
    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he1)
        .status()
        .unwrap();
    assert!(status.success(), "encode a failed");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he2)
        .status()
        .unwrap();
    assert!(status.success(), "encode b failed");

    // Cat indexed compressed files
    let status = hecate()
        .args(["cat", "-i"])
        .arg(&he1)
        .arg(&he2)
        .args(["-o"])
        .arg(&cat_he)
        .status()
        .unwrap();
    assert!(status.success(), "cat indexed compressed failed");

    // Decode and verify
    let status = hecate()
        .args(["-d"])
        .arg(&cat_he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "decode cat result failed");

    let decoded = fs::read_to_string(&fq).unwrap();
    let record_count = decoded.lines().filter(|l| l.starts_with('@')).count();
    assert_eq!(record_count, 8, "expected 8 records after cat");

    // Verify all headers are correctly reconstructed
    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let orig_headers: Vec<&str> = original.lines().step_by(4).collect();
    let dec_headers: Vec<&str> = decoded.lines().step_by(4).collect();
    for (i, oh) in orig_headers.iter().enumerate() {
        assert_eq!(*oh, dec_headers[i], "first copy header {} mismatch", i);
        assert_eq!(*oh, dec_headers[i + 4], "second copy header {} mismatch", i);
    }
}

// ---------------------------------------------------------------------------
// Auto-rANS pattern validation
// ---------------------------------------------------------------------------

#[test]
fn illumina_auto_rans_round_trip() {
    // When -L is given without -M, auto-rANS should be applied.
    // Verify the round-trip works with the auto-generated pattern.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-2", "-L", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "auto-rans encode failed");

    // Check that compression was actually applied (via stats)
    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success());
    let stats = String::from_utf8(output.stdout).unwrap();
    assert!(
        stats.contains("rans"),
        "auto-rANS should apply rans compression"
    );

    // Decode and verify
    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "auto-rans decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "auto-rans round-trip mismatch");
}

#[test]
fn illumina_auto_rans_indexed_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    // -L -I without -M: auto-rANS + indexed
    let status = hecate()
        .args(["-e", "-2", "-L", "-I", &data_path("illumina.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "auto-rans indexed encode failed");

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "auto-rans indexed decode failed");

    let original = fs::read_to_string(data_path("illumina.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "auto-rans indexed round-trip mismatch");
}

// ---------------------------------------------------------------------------
// Bare -I flag (default tier) tests
// ---------------------------------------------------------------------------

#[test]
fn illumina_bare_index_flag_defaults_to_full() {
    // Verify that -I without a value defaults to "full" tier
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-L",
            "-I",
            "-M",
            "",
            &data_path("illumina.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "bare -I encode failed");

    // Stats should show "indexed"
    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success());
    let stats = String::from_utf8(output.stdout).unwrap();
    assert!(
        stats.contains("Indexed: yes"),
        "file should be indexed with bare -I"
    );
}
