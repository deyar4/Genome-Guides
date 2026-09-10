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

use common::{data_path, hecate, normalize_fasta};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// FASTA round-trip tests
// ---------------------------------------------------------------------------

#[test]
fn fasta_2bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

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
    assert_eq!(original, decoded, "FASTA 2-bit round-trip mismatch");
}

#[test]
fn fasta_4bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-4", &data_path("small.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

#[test]
fn fasta_8bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-8", &data_path("small.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

#[test]
fn fasta_iupac_4bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-4", &data_path("iupac.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("iupac.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// Mixed-case round-trip
// ---------------------------------------------------------------------------

#[test]
fn fasta_mixed_case_2bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("mixed_case.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("mixed_case.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

#[test]
fn fasta_mixed_case_8bit_joint_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-8", "-J", &data_path("mixed_case.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("mixed_case.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// FASTQ round-trip
// ---------------------------------------------------------------------------

#[test]
fn fastq_2bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded);
}

#[test]
fn fastq_4bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args(["-e", "-4", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let status = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success());

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// No compression
// ---------------------------------------------------------------------------

#[test]
fn fasta_no_compression_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
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

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded);
}

// ---------------------------------------------------------------------------
// Ignore case
// ---------------------------------------------------------------------------

#[test]
fn fasta_ignore_case_uppercases() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", "-C", &data_path("mixed_case.fa"), "-o"])
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

    let decoded = String::from_utf8(output.stdout).unwrap();
    // With ignore-case, all bases should be uppercase
    for line in decoded.lines() {
        if !line.starts_with('>') {
            assert_eq!(
                line,
                line.to_uppercase(),
                "case-ignore should produce all uppercase"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Large-file round-trip (E. coli genome, MARV FASTQ)
// ---------------------------------------------------------------------------

#[test]
fn ecoli_2bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "decode failed");

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "E. coli 2-bit round-trip mismatch");
}

#[test]
fn ecoli_4bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let status = hecate()
        .args(["-e", "-4", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "decode failed");

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "E. coli 4-bit round-trip mismatch");
}

#[test]
fn ecoli_2bit_zstd_round_trip() {
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
    assert!(status.success(), "encode failed");

    let output = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "decode failed");

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap());
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "E. coli 2-bit+zstd round-trip mismatch");
}

#[test]
fn fastq_marv_2bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("marv.he");

    let status = hecate()
        .args(["-e", "-2", "-Q", &data_path("SRR25496580-MARV.fastq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate().args(["-d", "-c"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "decode failed");

    let original = fs::read_to_string(data_path("SRR25496580-MARV.fastq")).unwrap();
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "MARV FASTQ 2-bit round-trip mismatch");
}

#[test]
fn fastq_marv_2bit_zstd_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("marv.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-Q",
            "-M",
            "zstd:3",
            &data_path("SRR25496580-MARV.fastq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate().args(["-d", "-c"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "decode failed");

    let original = fs::read_to_string(data_path("SRR25496580-MARV.fastq")).unwrap();
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        original, decoded,
        "MARV FASTQ 2-bit+zstd round-trip mismatch"
    );
}

// ---------------------------------------------------------------------------
// Line wrapping
// ---------------------------------------------------------------------------

#[test]
fn fasta_default_line_width_80() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success());

    let output = hecate().args(["-d", "-c"]).arg(&he).output().unwrap();
    assert!(output.status.success());

    let decoded = String::from_utf8(output.stdout).unwrap();
    // All sequence lines should be <= 80 characters
    for line in decoded.lines() {
        if !line.starts_with('>') {
            assert!(line.len() <= 80, "line too long: {} chars", line.len());
        }
    }
}

// ---------------------------------------------------------------------------
// Indexed decode round-trip (encode with index, decode fully, compare)
// ---------------------------------------------------------------------------

#[test]
fn indexed_full_decode_roundtrip_k12_zstd() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");
    let fa = tmp.path().join("ecoli.fa");

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

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success());

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(original, decoded, "indexed K-12 full decode mismatch");
}

#[test]
fn indexed_full_decode_roundtrip_sakai_zstd() {
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
    assert_eq!(original, decoded, "indexed Sakai full decode mismatch");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: BWT compression
// ---------------------------------------------------------------------------

#[test]
fn fasta_2bit_bwt_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "bwt:4",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "BWT encode failed");

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success(), "BWT decode failed");

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn fasta_4bit_bwt_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args([
            "-e",
            "-4",
            "-I=full",
            "-M",
            "bwt:4",
            &data_path("iupac.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "BWT 4-bit encode failed");

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success(), "BWT 4-bit decode failed");

    let original = fs::read_to_string(data_path("iupac.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn verify_bwt_compressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "bwt:4",
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
        "verify BWT failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn stats_bwt_compressed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "bwt:4",
            &data_path("small.fa"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(
        o.status.success(),
        "stats BWT failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn cat_bwt_indexed() {
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
                "bwt:4",
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
        "cat BWT failed: {}",
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
    assert_eq!(decoded, format!("{}{}", original, original));
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: FASTQ paths
// ---------------------------------------------------------------------------

#[test]
fn fastq_8bit_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let s = hecate()
        .args(["-e", "-8", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "FASTQ 8-bit encode failed");

    let s = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(s.success(), "FASTQ 8-bit decode failed");

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn fastq_indexed_slice_by_name() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Slice a specific read by regex
    let o = hecate()
        .args(["-S", "/read1/", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "FASTQ indexed slice failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let decoded = String::from_utf8(o.stdout).unwrap();
    assert!(decoded.contains("@read1"), "should contain read1");
    assert!(!decoded.contains("@read2"), "should not contain read2");
}

#[test]
fn fastq_indexed_zstd_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-I=full",
            "-M",
            "zstd:3",
            &data_path("small.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let s = hecate()
        .args(["-d"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn fastq_list_indexed() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fq"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate().args(["-l"]).arg(&he).output().unwrap();
    assert!(o.status.success());
    let stdout = String::from_utf8(o.stdout).unwrap();
    assert!(stdout.contains("read1"), "list should show read1");
}

// ---------------------------------------------------------------------------
// Coverage-driven tests: 8-bit FASTA encodings
// ---------------------------------------------------------------------------

#[test]
fn fasta_8bit_ignore_case_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args(["-e", "-8", "-C", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success());

    // With -C, all output should be uppercase
    let decoded = fs::read_to_string(&fa).unwrap();
    for line in decoded.lines() {
        if !line.starts_with('>') {
            assert_eq!(line, line.to_uppercase(), "should be uppercased with -C");
        }
    }
}

#[test]
fn fasta_8bit_joint_case_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fa = tmp.path().join("out.fa");

    let s = hecate()
        .args(["-e", "-8", "-J", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "8-bit joint-case encode failed");

    let s = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(s.success());

    let original = fs::read_to_string(data_path("mixed_case.fa")).unwrap();
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(decoded, original);
}
