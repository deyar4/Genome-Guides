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
// Referential compression tests
// ---------------------------------------------------------------------------

#[test]
fn ref_encode_decode_2bit_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let ref_he = tmp.path().join("ref.he");
    let input_he = tmp.path().join("input.he");
    let diff_he = tmp.path().join("diff.he");
    let recon_he = tmp.path().join("recon.he");
    let out_fa = tmp.path().join("out.fa");

    assert!(hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&ref_he)
        .status()
        .unwrap()
        .success());

    fs::copy(&ref_he, &input_he).unwrap();

    assert!(hecate()
        .args(["-E", "-R"])
        .arg(&ref_he)
        .arg(&input_he)
        .args(["-o"])
        .arg(&diff_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-D", "-R"])
        .arg(&ref_he)
        .arg(&diff_he)
        .args(["-o"])
        .arg(&recon_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&recon_he)
        .args(["-o"])
        .arg(&out_fa)
        .status()
        .unwrap()
        .success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&out_fa).unwrap();
    assert_eq!(original, decoded, "referential 2-bit round-trip mismatch");
}

#[test]
fn ref_encode_decode_2bit_zstd_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let ref_he = tmp.path().join("ref.he");
    let input_he = tmp.path().join("input.he");
    let diff_he = tmp.path().join("diff.he");
    let recon_he = tmp.path().join("recon.he");
    let out_fa = tmp.path().join("out.fa");

    assert!(hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&ref_he)
        .status()
        .unwrap()
        .success());

    fs::copy(&ref_he, &input_he).unwrap();

    assert!(hecate()
        .args(["-E", "-R"])
        .arg(&ref_he)
        .arg(&input_he)
        .args(["-o"])
        .arg(&diff_he)
        .args(["-M", "zstd:3"])
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-D", "-R"])
        .arg(&ref_he)
        .arg(&diff_he)
        .args(["-o"])
        .arg(&recon_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&recon_he)
        .args(["-o"])
        .arg(&out_fa)
        .status()
        .unwrap()
        .success());

    let original = fs::read_to_string(data_path("small.fa")).unwrap();
    let decoded = fs::read_to_string(&out_fa).unwrap();
    assert_eq!(
        original, decoded,
        "referential 2-bit zstd round-trip mismatch"
    );
}

#[test]
fn ref_encode_decode_4bit_iupac_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let ref_he = tmp.path().join("ref.he");
    let input_he = tmp.path().join("input.he");
    let diff_he = tmp.path().join("diff.he");
    let recon_he = tmp.path().join("recon.he");
    let out_fa = tmp.path().join("out.fa");

    assert!(hecate()
        .args(["-e", "-4", &data_path("iupac.fa"), "-o"])
        .arg(&ref_he)
        .status()
        .unwrap()
        .success());

    fs::copy(&ref_he, &input_he).unwrap();

    assert!(hecate()
        .args(["-E", "-R"])
        .arg(&ref_he)
        .arg(&input_he)
        .args(["-o"])
        .arg(&diff_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-D", "-R"])
        .arg(&ref_he)
        .arg(&diff_he)
        .args(["-o"])
        .arg(&recon_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&recon_he)
        .args(["-o"])
        .arg(&out_fa)
        .status()
        .unwrap()
        .success());

    let original = fs::read_to_string(data_path("iupac.fa")).unwrap();
    let decoded = fs::read_to_string(&out_fa).unwrap();
    assert_eq!(
        original, decoded,
        "referential 4-bit IUPAC round-trip mismatch"
    );
}

#[test]
fn ref_encode_decode_ecoli_multiblock_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let ref_he = tmp.path().join("ref.he");
    let input_he = tmp.path().join("input.he");
    let diff_he = tmp.path().join("diff.he");
    let recon_he = tmp.path().join("recon.he");
    let orig_fa = tmp.path().join("orig.fa");
    let recon_fa = tmp.path().join("recon.fa");

    assert!(hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&ref_he)
        .status()
        .unwrap()
        .success());

    fs::copy(&ref_he, &input_he).unwrap();

    assert!(hecate()
        .args(["-E", "-R"])
        .arg(&ref_he)
        .arg(&input_he)
        .args(["-o"])
        .arg(&diff_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-D", "-R"])
        .arg(&ref_he)
        .arg(&diff_he)
        .args(["-o"])
        .arg(&recon_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&ref_he)
        .args(["-o"])
        .arg(&orig_fa)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&recon_he)
        .args(["-o"])
        .arg(&recon_fa)
        .status()
        .unwrap()
        .success());

    let original = fs::read_to_string(&orig_fa).unwrap();
    let decoded = fs::read_to_string(&recon_fa).unwrap();
    assert_eq!(
        original, decoded,
        "referential E. coli multi-block round-trip mismatch"
    );
}

#[test]
fn ref_encode_decode_cross_genome_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let ref_he = tmp.path().join("ref.he");
    let input_he = tmp.path().join("input.he");
    let diff_he = tmp.path().join("diff.he");
    let recon_he = tmp.path().join("recon.he");
    let orig_fa = tmp.path().join("orig.fa");
    let recon_fa = tmp.path().join("recon.fa");

    assert!(hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&ref_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-e", "-2", &data_path("GCF_000008865.2.fna"), "-o"])
        .arg(&input_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-E", "-R"])
        .arg(&ref_he)
        .arg(&input_he)
        .args(["-o"])
        .arg(&diff_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-D", "-R"])
        .arg(&ref_he)
        .arg(&diff_he)
        .args(["-o"])
        .arg(&recon_he)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&input_he)
        .args(["-o"])
        .arg(&orig_fa)
        .status()
        .unwrap()
        .success());

    assert!(hecate()
        .args(["-d", "-w", "0"])
        .arg(&recon_he)
        .args(["-o"])
        .arg(&recon_fa)
        .status()
        .unwrap()
        .success());

    let original = fs::read_to_string(&orig_fa).unwrap();
    let decoded = fs::read_to_string(&recon_fa).unwrap();
    assert_eq!(
        original, decoded,
        "referential cross-genome round-trip mismatch"
    );
}

// ---------------------------------------------------------------------------
// rANS compression round-trip tests
// ---------------------------------------------------------------------------

#[test]
fn fastq_rans_quality_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "rans",
            "-P",
            "_____0",
            &data_path("small.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "rans encode failed");

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "rans decode failed");

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "rANS FASTQ round-trip mismatch");
}

#[test]
fn fastq_rans_zstd_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3,rans",
            "-P",
            "00_0_1",
            &data_path("small.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "zstd+rans encode failed");

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "zstd+rans decode failed");

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "zstd+rANS FASTQ round-trip mismatch");
}

#[test]
fn fastq_rans_indexed_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");
    let fq = tmp.path().join("out.fq");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3,rans",
            "-P",
            "00_0_1",
            "-I=full",
            &data_path("small.fq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "indexed rans encode failed");

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fq)
        .status()
        .unwrap();
    assert!(status.success(), "indexed rans decode failed");

    let original = fs::read_to_string(data_path("small.fq")).unwrap();
    let decoded = fs::read_to_string(&fq).unwrap();
    assert_eq!(original, decoded, "indexed rANS FASTQ round-trip mismatch");
}

#[test]
fn fastq_marv_rans_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("marv.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3,rans",
            "-P",
            "00_0_1",
            &data_path("SRR25496580-MARV.fastq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "MARV rans encode failed");

    let output = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "MARV rans decode failed");

    let original = fs::read_to_string(data_path("SRR25496580-MARV.fastq")).unwrap();
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "MARV rANS round-trip mismatch");
}

#[test]
fn fastq_marv_rans_indexed_round_trip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("marv.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3,rans",
            "-P",
            "00_0_1",
            "-I=full",
            &data_path("SRR25496580-MARV.fastq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "MARV indexed rans encode failed");

    let output = hecate()
        .args(["-d", "-w", "0", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "MARV indexed rans decode failed");

    let original = fs::read_to_string(data_path("SRR25496580-MARV.fastq")).unwrap();
    let decoded = String::from_utf8(output.stdout).unwrap();
    assert_eq!(original, decoded, "MARV indexed rANS round-trip mismatch");
}

#[test]
fn fastq_rans_stats_show_compression() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3,rans",
            "-P",
            "00_0_1",
            &data_path("SRR25496580-MARV.fastq"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "encode failed");

    let output = hecate().args(["stats"]).arg(&he).output().unwrap();
    assert!(output.status.success(), "stats failed");

    let stats = String::from_utf8(output.stdout).unwrap();
    assert!(
        stats.contains("rans"),
        "stats should mention rans compression"
    );
    assert!(
        stats.contains("QUALITY"),
        "stats should show QUALITY stream"
    );
}

// ---------------------------------------------------------------------------
// Indexed Markov decompressor reset tests
// ---------------------------------------------------------------------------

#[test]
fn indexed_markov_reset_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("markov_indexed.he");
    let fa = tmp.path().join("decoded.fa");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "markov",
            "-P",
            "_0___",
            "-I=full,1",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "markov indexed encode failed");

    let status = hecate()
        .args(["-d", "-w", "0"])
        .arg(&he)
        .args(["-o"])
        .arg(&fa)
        .status()
        .unwrap();
    assert!(status.success(), "markov indexed decode failed");

    let original = normalize_fasta(&fs::read_to_string(data_path("GCF_000005845.2.fna")).unwrap());
    let decoded = fs::read_to_string(&fa).unwrap();
    assert_eq!(
        original, decoded,
        "indexed Markov round-trip mismatch -- decompressor reset likely broken"
    );
}

#[test]
fn indexed_markov_slice_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("markov_indexed.he");

    let status = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "markov",
            "-P",
            "_0___",
            "-I=full,1",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he)
        .status()
        .unwrap();
    assert!(status.success(), "markov indexed encode failed");

    let output = hecate()
        .args(["-S", "#0:100001:200000", "-c", "-w", "0"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(output.status.success(), "markov indexed slice failed");

    let sliced = String::from_utf8(output.stdout).unwrap();
    assert!(
        sliced.starts_with('>'),
        "sliced output should start with FASTA header"
    );
    let seq: String = sliced.lines().skip(1).collect();
    assert_eq!(
        seq.len(),
        100000,
        "sliced sequence should be exactly 100000 bases, got {}",
        seq.len()
    );
}
