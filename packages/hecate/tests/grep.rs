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
// Grep (query) tests
// ---------------------------------------------------------------------------

/// Encode a file, then run grep and return the raw stdout.
fn encode_grep(input_file: &str, alphabet: &str, grep_args: &[&str]) -> String {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", alphabet, input_file, "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "encode failed");

    let mut cmd = hecate();
    cmd.arg("-c");
    for a in grep_args {
        cmd.arg(a);
    }
    cmd.arg(&he);
    let o = cmd.output().unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}

/// Parse CSV output into Vec of rows (each row = Vec of fields).
/// Skips the header row.
fn parse_csv_rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1) // header
        .filter(|l| !l.is_empty())
        .map(|line| line.split(',').map(String::from).collect())
        .collect()
}

#[test]
fn grep_basic_global() {
    // seq1 = ACGTACGTACGTACGT, seq2 = AAAACCCCGGGGTTTTACGTACGTACGTACGT
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/ACGT/g"]);
    let rows = parse_csv_rows(&out);
    // seq1 has 4 ACGT at 0,4,8,12; seq2 has 4 ACGT at 16,20,24,28
    assert_eq!(rows.len(), 8);
    // First match: seq1, pos 0, global 0
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[0][2], "0");
    assert_eq!(rows[0][3], "ACGT");
    // First seq2 match: pos 16, global 16+16=32
    assert_eq!(rows[4][1], "16");
    assert_eq!(rows[4][2], "32");
}

#[test]
fn grep_first_match_only() {
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/ACGT/"]);
    let rows = parse_csv_rows(&out);
    // Without /g, one match per sequence
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1], "0"); // seq1 pos 0
    assert_eq!(rows[1][1], "16"); // seq2 pos 16
}

#[test]
fn grep_no_match_empty_output() {
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/NNNN/g"]);
    // No matches => no header, no rows
    assert!(out.is_empty(), "expected empty output, got: {}", out);
}

#[test]
fn grep_case_insensitive() {
    // mixed_case.fa: gene1 = ACGTacgtACGT, gene2 = acgtacgtacgt
    let out = encode_grep(&data_path("mixed_case.fa"), "-2", &["-G", "/ACGT/ig"]);
    let rows = parse_csv_rows(&out);
    // gene1: 3 matches (ACGT, acgt, ACGT), gene2: 3 matches (acgt, acgt, acgt)
    assert_eq!(rows.len(), 6);
    // Verify case is preserved in matched text
    assert_eq!(rows[1][3], "acgt"); // gene1 second match is lowercase
}

#[test]
fn grep_case_sensitive_skips_lowercase() {
    let out = encode_grep(&data_path("mixed_case.fa"), "-2", &["-G", "/ACGT/g"]);
    let rows = parse_csv_rows(&out);
    // Only uppercase ACGT matches: gene1 pos 0 and 8
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][3], "ACGT");
    assert_eq!(rows[1][3], "ACGT");
}

#[test]
fn grep_wildcard() {
    // seq1 = ACGTACGTACGTACGT
    // /A##T/ matches ACGT at every 4-base boundary
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/A##T/g"]);
    let rows = parse_csv_rows(&out);
    assert!(rows.len() >= 4, "should match A..T patterns");
    for row in &rows {
        let m = &row[3];
        assert!(
            m.starts_with('A') && m.ends_with('T'),
            "wildcard match should be A..T: {}",
            m
        );
        assert_eq!(m.len(), 4);
    }
}

#[test]
fn grep_neighbourhood() {
    // seq1 = ACGTACGTACGTACGT (16 bases)
    // Match ACGT at pos 4 with neighbourhood 2
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/ACGT/", "-n", "2"]);
    assert!(
        out.contains("left,right"),
        "header should have left,right columns"
    );
    let rows = parse_csv_rows(&out);
    // First match: seq1 pos 0, left clamped (only 0 bases available)
    assert_eq!(rows[0][4], ""); // left empty at start of sequence
    assert_eq!(rows[0][5], "AC"); // right = 2 bases after ACGT
}

#[test]
fn grep_neighbourhood_end_clamping() {
    // seq1 = ACGTACGTACGTACGT (16 bases)
    // Match ACGT at pos 12 (last occurrence), right context clamped
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/ACGT/g", "-n", "3"]);
    let rows = parse_csv_rows(&out);
    // Find seq1 match at pos 12
    let last_seq1 = rows.iter().find(|r| r[1] == "12").unwrap();
    assert_eq!(last_seq1[4], "CGT"); // left = 3 bases before pos 12
    assert_eq!(last_seq1[5], ""); // right empty -- ACGT ends at pos 16 = end of seq
}

#[test]
fn grep_4bit_alphabet() {
    let out = encode_grep(&data_path("small.fa"), "-4", &["-G", "/CCCC/g"]);
    let rows = parse_csv_rows(&out);
    // seq2 = AAAACCCCGGGGTTTT..., CCCC at pos 4
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "4");
    assert_eq!(rows[0][3], "CCCC");
}

#[test]
fn grep_8bit_alphabet() {
    let out = encode_grep(&data_path("small.fa"), "-8", &["-G", "/GGGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "8"); // seq2 pos 8
    assert_eq!(rows[0][3], "GGGG");
}

#[test]
fn grep_fastq() {
    // small.fq: read1 = ACGTACGT, read2 = GGGGCCCC
    let out = encode_grep(&data_path("small.fq"), "-2", &["-G", "/ACGT/g"]);
    let rows = parse_csv_rows(&out);
    // read1 has ACGT at pos 0 and 4
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[1][1], "4");
}

#[test]
fn grep_global_pos_across_sequences() {
    // seq1 = 16 bases, seq2 = 32 bases
    let out = encode_grep(&data_path("small.fa"), "-2", &["-G", "/TTTT/g"]);
    let rows = parse_csv_rows(&out);
    // TTTT appears in seq2 at pos 12 (GGGGTTTT), global_pos = 16 + 12 = 28
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "12"); // seq_pos
    assert_eq!(rows[0][2], "28"); // global_pos = 16 (seq1 len) + 12
}

#[test]
fn grep_with_slice_nuc_skip_no_spurious_match() {
    // This is the critical nuc_skip test.
    // seq2 = AAAACCCCGGGGTTTTACGTACGTACGTACGT (32 bases)
    // Slice from base 4 (1-based) = base 3 (0-based).
    // With 2-bit: nuc_skip = 3 % 4 = 3. Three padding bases prepended.
    // Packed byte 0 has original bases 0-3: A,A,A,A.
    // After unpacking with padding: [A,A,A, A,C,C,C,C,G,G,G,G,...]
    //                                 pad    real data starts here
    // Without nuc_skip handling, "AAAC" would match at index 0 (spurious!).
    // With correct handling, search starts at index 3: "ACCCC..." -- no "AAAC".
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/AAAC/g", "-S", "seq2 second sequence:4:32", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    // "AAAC" must NOT appear -- the sliced region starts with "ACCCC..."
    assert!(
        out.is_empty(),
        "nuc_skip bug: spurious match in padding region: {}",
        out,
    );
}

#[test]
fn grep_with_slice_valid_match() {
    // Complement to the above: verify that a valid match in the sliced
    // region IS found.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Sliced region (bases 4..32, 1-based) = ACCCCGGGGTTTTACGTACGTACGTACGT
    // CCCC should match at position 1 within the slice
    let o = hecate()
        .args(["-G", "/CCCC/g", "-S", "seq2 second sequence:4:32", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "1"); // pos 1 within sliced region
}

#[test]
fn grep_with_slice_neighbourhood_no_padding_leak() {
    // Verify that neighbourhood context does not include padding bases.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Slice seq2 from base 4 (1-based, nuc_skip=3 in 2-bit).
    // Sliced text: ACCCCGGGGTTTTACGTACGTACGTACGT (29 bases)
    // CCCC at pos 1. With neighbourhood 5, left should be "A" (only 1 base
    // before the match, not padding).
    let o = hecate()
        .args([
            "-G",
            "/CCCC/g",
            "-S",
            "seq2 second sequence:4:32",
            "-n",
            "5",
            "-c",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    // Left context: only "A" available (1 base before pos 1)
    assert_eq!(rows[0][4], "A");
    // Right context: 5 bases after CCCC = "GGGGT"
    assert_eq!(rows[0][5], "GGGGT");
}

#[test]
fn grep_with_slice_4bit_nuc_skip() {
    // 4-bit: 2 bases per byte. Slice from odd position -> nuc_skip = 1.
    // seq1 = ACGTACGTACGTACGT
    // Slice from base 2 (1-based) = base 1 (0-based). nuc_skip = 1 % 2 = 1.
    // Sliced text: CGTACGTACGTACGT (15 bases)
    // "AC" should NOT match at position 0 (that would be padding A + real C).
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-4", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/AC/", "-S", "seq1 first sequence:2:16", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // Sliced text: CGTACGTACGTACGT
    // First "AC" is at position 3 (CGT_AC_...), not position 0
    assert!(!rows.is_empty(), "should find AC in sliced region");
    assert_eq!(rows[0][1], "3", "first AC should be at pos 3, not 0");
}

#[test]
fn grep_subcommand_form() {
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["query", "-G", "/CCCC/g", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][3], "CCCC");
}

#[test]
fn grep_csv_header_quoting() {
    // The E. coli header contains a comma, so it must be quoted in CSV.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("ecoli.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/GAATTC/", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(o.status.success());
    let out = String::from_utf8(o.stdout).unwrap();
    // Header contains comma -> must be double-quoted
    let data_line = out.lines().nth(1).unwrap();
    assert!(
        data_line.starts_with('"'),
        "seq_name with comma should be quoted: {}",
        data_line,
    );
}

#[test]
fn grep_compressed_matches_uncompressed() {
    // Grep results must be identical regardless of compression.
    let tmp = TempDir::new().unwrap();
    let he_raw = tmp.path().join("raw.he");
    let he_zstd = tmp.path().join("zstd.he");

    let s = hecate()
        .args(["-e", "-2", &data_path("GCF_000005845.2.fna"), "-o"])
        .arg(&he_raw)
        .status()
        .unwrap();
    assert!(s.success());

    let s = hecate()
        .args([
            "-e",
            "-2",
            "-M",
            "zstd:3",
            &data_path("GCF_000005845.2.fna"),
            "-o",
        ])
        .arg(&he_zstd)
        .status()
        .unwrap();
    assert!(s.success());

    let o1 = hecate()
        .args(["-G", "/GAATTC/g", "-c"])
        .arg(&he_raw)
        .output()
        .unwrap();
    let o2 = hecate()
        .args(["-G", "/GAATTC/g", "-c"])
        .arg(&he_zstd)
        .output()
        .unwrap();
    assert!(o1.status.success());
    assert!(o2.status.success());

    assert_eq!(
        String::from_utf8(o1.stdout).unwrap(),
        String::from_utf8(o2.stdout).unwrap(),
        "compressed vs uncompressed grep results differ",
    );
}

#[test]
fn grep_extras_iupac_2bit() {
    // With 2-bit alphabet, IUPAC codes (R, Y, S, W, etc.) are out-of-alphabet
    // and stored as extras. Verify that grep matches patterns containing
    // reconstructed extra bytes.
    // iupac.fa: ambig1 = ACGTRYSWKMBDHVN
    let out = encode_grep(&data_path("iupac.fa"), "-2", &["-G", "/RYSW/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "4"); // RYSW at pos 4
    assert_eq!(rows[0][3], "RYSW");
}

#[test]
fn grep_wildcard_across_extras() {
    // Wildcard matching across a mix of normal and extra-reconstructed bases.
    // iupac.fa: ambig1 = ACGTRYSWKMBDHVN
    // Pattern A###R: matches ACGTR at pos 0
    let out = encode_grep(&data_path("iupac.fa"), "-2", &["-G", "/A###R/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[0][3], "ACGTR");
}

#[test]
fn grep_slice_with_extras_no_index() {
    // Grep inside a sliced region that contains out-of-alphabet (extra) bases,
    // without an index (sequential scan path).
    //
    // iupac.fa: ambig1 = ACGTRYSWKMBDHVN (15 bases)
    // 2-bit alphabet: R,Y,S,W,K,M,B,D,H,V,N are all extras.
    // Slice bases 3..10 (1-based inclusive) = 0-based [2,10) = "GTRYSWKM"
    // nuc_skip = 2 % 4 = 2 for 2-bit.
    // Search for "RYSW" -- should match at position 2 within the slice.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // Encode WITHOUT index
    let s = hecate()
        .args(["-e", "-2", &data_path("iupac.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args([
            "-G",
            "/RYSW/g",
            "-S",
            "ambig1 IUPAC ambiguity codes:3:10",
            "-c",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected 1 match, got: {}", out);
    // Sliced text is GTRYSWKM (8 bases). RYSW at pos 2.
    assert_eq!(rows[0][1], "2", "RYSW should be at pos 2 in slice");
    assert_eq!(rows[0][3], "RYSW");
}

#[test]
fn grep_slice_with_extras_indexed() {
    // Same as above but with a full index, exercising the indexed slice path.
    //
    // iupac.fa: ambig1 = ACGTRYSWKMBDHVN (15 bases)
    // Slice bases 3..10 (1-based inclusive) = "GTRYSWKM"
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    // Encode WITH full index
    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("iupac.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args([
            "-G",
            "/RYSW/g",
            "-S",
            "ambig1 IUPAC ambiguity codes:3:10",
            "-c",
        ])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected 1 match, got: {}", out);
    assert_eq!(rows[0][1], "2", "RYSW should be at pos 2 in slice");
    assert_eq!(rows[0][3], "RYSW");
}

#[test]
fn grep_8bit_joint_case_preserved() {
    // mixed_case.fa: gene1 = ACGTacgtACGT, gene2 = acgtacgtacgt
    // 8-bit Joint mode: case baked into nuc stream. Fast path should
    // search directly and preserve original case in output.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-8", "-J", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "encode failed");

    // Case-insensitive search: should find "ACGT" and "acgt" variants
    let o = hecate()
        .args(["-G", "/ACGT/ig", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // gene1: ACGT at 0, acgt at 4, ACGT at 8 = 3 matches
    // gene2: acgt at 0, acgt at 4, acgt at 8 = 3 matches
    assert_eq!(rows.len(), 6, "expected 6 matches, got: {}", out);
    // Original case preserved: first match is uppercase ACGT
    assert_eq!(rows[0][3], "ACGT");
    // Second match is lowercase acgt
    assert_eq!(rows[1][3], "acgt");
}

#[test]
fn grep_8bit_joint_case_sensitive() {
    // 8-bit Joint, case-sensitive search: only uppercase ACGT should match
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-8", "-J", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/ACGT/g", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // gene1: ACGT at 0 and 8 = 2 matches. gene2: none (all lowercase)
    assert_eq!(
        rows.len(),
        2,
        "expected 2 case-sensitive matches, got: {}",
        out
    );
    assert_eq!(rows[0][0], "gene1 uppercase and lowercase");
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[1][1], "8");
}

#[test]
fn grep_8bit_include_case_sensitive() {
    // 8-bit Include mode (default, no -J or -C): case in separate stream.
    // Case-sensitive search should respect original case via slow path.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-8", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    // Case-sensitive: only uppercase ACGT
    let o = hecate()
        .args(["-G", "/ACGT/g", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // gene1 = ACGTacgtACGT: uppercase ACGT at 0 and 8
    // gene2 = acgtacgtacgt: none
    assert_eq!(rows.len(), 2, "expected 2 matches, got: {}", out);
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[1][1], "8");
}

#[test]
fn grep_8bit_include_case_insensitive() {
    // 8-bit Include, case-insensitive: should find all ACGT variants
    // and preserve original case in output (goes through slow path).
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-8", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/ACGT/ig", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // gene1: 3 matches, gene2: 3 matches = 6
    assert_eq!(rows.len(), 6, "expected 6 matches, got: {}", out);
    // Original case preserved in output
    assert_eq!(rows[0][3], "ACGT");
    assert_eq!(rows[1][3], "acgt");
    assert_eq!(rows[3][3], "acgt"); // gene2 first match
}

#[test]
fn grep_8bit_ignore_case() {
    // 8-bit Ignore mode (-C): case discarded, all stored uppercase.
    // Fast path should work; case-insensitive flag is irrelevant
    // since everything is uppercase.
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("out.he");

    let s = hecate()
        .args(["-e", "-8", "-C", &data_path("mixed_case.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/ACGT/g", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // Everything uppercase, so ACGT matches everywhere:
    // gene1: ACGTACGTACGT -> 3 matches (0, 4, 8)
    // gene2: ACGTACGTACGT -> 3 matches (0, 4, 8)
    assert_eq!(rows.len(), 6, "expected 6 matches, got: {}", out);
    // All uppercase in output since case was discarded
    for r in &rows {
        assert_eq!(r[3], "ACGT");
    }
}

// ---------------------------------------------------------------------------
// Grep: chunk-boundary straddling tests
//
// The source driver delivers packed nuc data in 65536-byte chunks.
// For 2-bit encoding that's 262144 bases per chunk.  These tests verify
// that the streaming sliding-window search correctly finds patterns
// that straddle the chunk boundary.
// ---------------------------------------------------------------------------

/// Helper: write a synthetic FASTA, encode it, and run grep.
fn boundary_grep(seq: &[u8], alphabet: &str, grep_args: &[&str]) -> String {
    let tmp = TempDir::new().unwrap();
    let fa = tmp.path().join("boundary.fa");
    let he = tmp.path().join("boundary.he");

    {
        use std::io::Write;
        let mut f = fs::File::create(&fa).unwrap();
        write!(f, ">seq\n").unwrap();
        f.write_all(seq).unwrap();
        write!(f, "\n").unwrap();
    }

    let s = hecate()
        .args(["-e", alphabet])
        .arg(&fa)
        .args(["-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "encode failed");

    let mut cmd = hecate();
    cmd.arg("-c");
    for a in grep_args {
        cmd.arg(a);
    }
    cmd.arg(&he);
    let o = cmd.output().unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap()
}

#[test]
fn grep_boundary_straddling_2bit() {
    // 2-bit chunk = 65536 bytes = 262144 bases.
    // Place "CCGG" at position 262142 so 2 bases are in chunk 1, 2 in chunk 2.
    let mut seq = vec![b'A'; 262200];
    seq[262142] = b'C';
    seq[262143] = b'C';
    seq[262144] = b'G';
    seq[262145] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected 1 boundary-straddling match, got: {}",
        out
    );
    assert_eq!(rows[0][1], "262142");
    assert_eq!(rows[0][3], "CCGG");
}

#[test]
fn grep_boundary_straddling_2bit_neighbourhood() {
    // Same setup but with neighbourhood=5.
    // Left context should be "AAAAA", right context "AAAAA".
    let mut seq = vec![b'A'; 262200];
    seq[262142] = b'C';
    seq[262143] = b'C';
    seq[262144] = b'G';
    seq[262145] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g", "-n", "5"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected 1 match, got: {}", out);
    assert_eq!(rows[0][1], "262142");
    assert_eq!(rows[0][3], "CCGG");
    assert_eq!(rows[0][4], "AAAAA"); // left context from chunk 1
    assert_eq!(rows[0][5], "AAAAA"); // right context from chunk 2
}

#[test]
fn grep_boundary_straddling_2bit_global_multi() {
    // Matches in chunk 1, on the boundary, and in chunk 2.
    let mut seq = vec![b'A'; 262200];
    // Match 1: entirely in chunk 1
    seq[100] = b'C';
    seq[101] = b'C';
    seq[102] = b'G';
    seq[103] = b'G';
    // Match 2: straddling boundary
    seq[262142] = b'C';
    seq[262143] = b'C';
    seq[262144] = b'G';
    seq[262145] = b'G';
    // Match 3: entirely in chunk 2
    seq[262180] = b'C';
    seq[262181] = b'C';
    seq[262182] = b'G';
    seq[262183] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 3, "expected 3 matches, got: {}", out);
    assert_eq!(rows[0][1], "100");
    assert_eq!(rows[1][1], "262142");
    assert_eq!(rows[2][1], "262180");
}

#[test]
fn grep_boundary_non_global_first_in_chunk2() {
    // No match in chunk 1, first (and only) match is in chunk 2.
    // Non-global mode should still find it.
    let mut seq = vec![b'A'; 262200];
    seq[262180] = b'C';
    seq[262181] = b'C';
    seq[262182] = b'G';
    seq[262183] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected 1 non-global match, got: {}", out);
    assert_eq!(rows[0][1], "262180");
}

#[test]
fn grep_boundary_straddling_8bit() {
    // 8-bit chunk = 65536 bytes = 65536 bases.
    // Place "CCGG" at position 65534 so it straddles the boundary.
    let mut seq = vec![b'A'; 65600];
    seq[65534] = b'C';
    seq[65535] = b'C';
    seq[65536] = b'G';
    seq[65537] = b'G';

    let out = boundary_grep(&seq, "-8", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected 1 boundary-straddling match, got: {}",
        out
    );
    assert_eq!(rows[0][1], "65534");
    assert_eq!(rows[0][3], "CCGG");
}

#[test]
fn grep_boundary_straddling_wildcard() {
    // Wildcard pattern "C#G" straddling the 2-bit chunk boundary.
    let mut seq = vec![b'A'; 262200];
    // Place C at last base of chunk 1, X (any) at boundary, G at first+1 of chunk 2
    seq[262143] = b'C';
    // seq[262144] = b'A' (filler -- wildcard matches it)
    seq[262145] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/C#G/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected 1 wildcard boundary match, got: {}",
        out
    );
    assert_eq!(rows[0][1], "262143");
    assert_eq!(rows[0][3], "CAG"); // C + A(wildcard) + G
}

#[test]
fn grep_multi_slice_nuc_skip_per_record() {
    // Verify that nuc_skip is correctly applied per-record when multiple
    // slices extract from different sequences.
    //
    // seq1: ACGTACGTACGTACGT (16 bases)
    // seq2: CCCCCCCCCCCCCCCC (filler, no match)
    // seq3: ACGTACGTACGTACGT (16 bases)
    // seq4: CCCCCCCCCCCCCCCC (filler, no match)
    //
    // Slice "seq1:3:8" and "seq3:3:8": 1-based [3,8] -> 0-based [2,8)
    // Sliced region: GTACGT (6 bases from each)
    // For 2-bit: nuc_skip = 2 % 4 = 2
    // Pattern ACGT found at position 2 within each sliced region.
    let tmp = TempDir::new().unwrap();
    let fa = tmp.path().join("multi.fa");
    let he = tmp.path().join("multi.he");

    fs::write(
        &fa,
        ">seq1\nACGTACGTACGTACGT\n\
         >seq2\nCCCCCCCCCCCCCCCC\n\
         >seq3\nACGTACGTACGTACGT\n\
         >seq4\nCCCCCCCCCCCCCCCC\n",
    )
    .unwrap();

    let s = hecate()
        .args(["-e", "-2"])
        .arg(&fa)
        .args(["-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "encode failed");

    let o = hecate()
        .args(["-G", "/ACGT/g", "-S", "seq1:3:8", "-S", "seq3:3:8", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);

    // Each sliced region is "GTACGT" (6 bases), ACGT match at position 2.
    assert_eq!(
        rows.len(),
        2,
        "expected 2 matches (one per slice), got: {}",
        out
    );
    assert_eq!(rows[0][0], "seq1");
    assert_eq!(rows[0][1], "2");
    assert_eq!(rows[0][3], "ACGT");
    assert_eq!(rows[1][0], "seq3");
    assert_eq!(rows[1][1], "2");
    assert_eq!(rows[1][3], "ACGT");

    // Global positions: seq1's sliced region is 6 real bases,
    // so seq3's match is at global 6 + 2 = 8.
    assert_eq!(rows[0][2], "2"); // seq1 global pos
    assert_eq!(rows[1][2], "8"); // seq3 global pos (6 + 2)
}

#[test]
fn grep_multi_slice_regex_nuc_skip() {
    // Same scenario as above but using a single regex slice spec
    // that matches both seq1 and seq3: /seq[13]/:3:8
    let tmp = TempDir::new().unwrap();
    let fa = tmp.path().join("multi.fa");
    let he = tmp.path().join("multi.he");

    fs::write(
        &fa,
        ">seq1\nACGTACGTACGTACGT\n\
         >seq2\nCCCCCCCCCCCCCCCC\n\
         >seq3\nACGTACGTACGTACGT\n\
         >seq4\nCCCCCCCCCCCCCCCC\n",
    )
    .unwrap();

    let s = hecate()
        .args(["-e", "-2"])
        .arg(&fa)
        .args(["-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success(), "encode failed");

    let o = hecate()
        .args(["-G", "/ACGT/g", "-S", "/seq[13]/:3:8", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);

    assert_eq!(rows.len(), 2, "expected 2 matches, got: {}", out);
    assert_eq!(rows[0][0], "seq1");
    assert_eq!(rows[0][1], "2");
    assert_eq!(rows[1][0], "seq3");
    assert_eq!(rows[1][1], "2");
    assert_eq!(rows[1][2], "8");
}

// ---------------------------------------------------------------------------
// Grep: additional edge-case tests
// ---------------------------------------------------------------------------

#[test]
fn grep_boundary_straddling_4bit() {
    // 4-bit chunk = 65536 bytes = 131072 bases per chunk.
    // Place "CCGG" at position 131070 so 2 bases in chunk 1, 2 in chunk 2.
    let mut seq = vec![b'A'; 131100];
    seq[131070] = b'C';
    seq[131071] = b'C';
    seq[131072] = b'G';
    seq[131073] = b'G';

    let out = boundary_grep(&seq, "-4", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected 1 boundary match in 4-bit, got: {}",
        out
    );
    assert_eq!(rows[0][1], "131070");
    assert_eq!(rows[0][3], "CCGG");
}

#[test]
fn grep_no_double_emission() {
    // Match near end of chunk 1 with full right context available.
    // It MUST be emitted in chunk 1 and NOT re-emitted from the overlap
    // zone when chunk 2 arrives.  Tests next_emit_pos / search_start.
    //
    // 2-bit: 262144 bases per chunk.  overlap = pat_len-1 + 2*nbr.
    // Pattern "CCGG" (4 bases), neighbourhood 0 -> overlap = 3.
    // Place match at 262130 -- well within chunk 1 with plenty of right
    // context (262144 - 262130 - 4 = 10 bases after the match).
    let mut seq = vec![b'A'; 262200];
    seq[262130] = b'C';
    seq[262131] = b'C';
    seq[262132] = b'G';
    seq[262133] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "match near chunk end emitted twice: {}", out);
    assert_eq!(rows[0][1], "262130");
}

#[test]
fn grep_non_global_first_match_straddles_boundary() {
    // Non-global mode: the very first match in the sequence straddles
    // the chunk boundary.  It can't be emitted in chunk 1 (no right
    // context), so it must be found in chunk 2.
    let mut seq = vec![b'A'; 262200];
    seq[262142] = b'C';
    seq[262143] = b'C';
    seq[262144] = b'G';
    seq[262145] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected exactly 1 non-global match, got: {}",
        out
    );
    assert_eq!(rows[0][1], "262142");
}

#[test]
fn grep_overlapping_matches_at_boundary() {
    // Global /AA/g over a run of A's that straddles the chunk boundary.
    // 2-bit chunk boundary at 262144.
    // Place 6 A's: 262141..262147.  This gives overlapping matches at
    // 262141, 262142, 262143, 262144, 262145 (5 total).
    // Some are in chunk 1, some straddle, some in chunk 2.
    let mut seq = vec![b'C'; 262200];
    for i in 262141..262147 {
        seq[i] = b'A';
    }
    // CCCC...CAAAAAAC...CCC
    // Overlapping "AA" matches: 262141, 262142, 262143, 262144, 262145

    let out = boundary_grep(&seq, "-2", &["-G", "/AA/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        5,
        "expected 5 overlapping AA matches at boundary, got: {}",
        out
    );
    for (i, expected_pos) in (262141..=262145).enumerate() {
        assert_eq!(
            rows[i][1],
            expected_pos.to_string(),
            "match {} at wrong position",
            i
        );
    }
}

#[test]
fn grep_match_at_exact_end_of_sequence() {
    // Match whose last base is the very last base of the sequence.
    // This exercises the is_last emit-all path.
    let mut seq = vec![b'A'; 262200];
    // Place CCGG at the very end: positions 262196..262200
    let end = seq.len();
    seq[end - 4] = b'C';
    seq[end - 3] = b'C';
    seq[end - 2] = b'G';
    seq[end - 1] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected match at exact end of seq, got: {}",
        out
    );
    assert_eq!(rows[0][1], (end - 4).to_string());
}

#[test]
fn grep_match_at_exact_chunk_boundary_start() {
    // Match starts at exactly the first base of chunk 2 (base 262144
    // in 2-bit mode).  Not straddling, but right at the edge.
    let mut seq = vec![b'A'; 262200];
    seq[262144] = b'C';
    seq[262145] = b'C';
    seq[262146] = b'G';
    seq[262147] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(
        rows.len(),
        1,
        "expected 1 match at chunk boundary start, got: {}",
        out
    );
    assert_eq!(rows[0][1], "262144");
}

#[test]
fn grep_sequence_shorter_than_pattern() {
    // A 3-base sequence searched with a 10-base pattern should give
    // zero matches and no panic.
    let seq = b"ACG";
    let out = boundary_grep(seq, "-2", &["-G", "/ACGTACGTAC/g"]);
    assert!(
        out.is_empty(),
        "expected no output for seq shorter than pattern, got: {}",
        out
    );
}

#[test]
fn grep_indexed_no_slice() {
    // Grep on a file encoded with -I=full but WITHOUT any -S slice.
    // Exercises a different mux reader path (indexed blocks).
    let tmp = TempDir::new().unwrap();
    let he = tmp.path().join("indexed.he");

    let s = hecate()
        .args(["-e", "-2", "-I=full", &data_path("small.fa"), "-o"])
        .arg(&he)
        .status()
        .unwrap();
    assert!(s.success());

    let o = hecate()
        .args(["-G", "/ACGT/g", "-c"])
        .arg(&he)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "grep failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8(o.stdout).unwrap();
    let rows = parse_csv_rows(&out);
    // Same as grep_basic_global: 8 ACGT matches
    assert_eq!(
        rows.len(),
        8,
        "indexed grep should match non-indexed, got: {}",
        out
    );
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[0][3], "ACGT");
}

#[test]
fn grep_boundary_neighbourhood_end_clamp() {
    // Match straddles the chunk boundary near the end of the sequence.
    // Right context should be clamped to available bases.
    //
    // Sequence length: 262150 bases (only 4 bases past the boundary match).
    // Match CCGG at 262142 (straddles). Right context request = 10 bases,
    // but only 262150 - 262146 = 4 bases available.
    let mut seq = vec![b'A'; 262150];
    seq[262142] = b'C';
    seq[262143] = b'C';
    seq[262144] = b'G';
    seq[262145] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g", "-n", "10"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected 1 match, got: {}", out);
    assert_eq!(rows[0][1], "262142");
    assert_eq!(rows[0][4], "AAAAAAAAAA"); // left: 10 A's available
    assert_eq!(rows[0][5], "AAAA"); // right: only 4 A's remain (clamped)
}

#[test]
fn grep_match_at_position_zero_multi_chunk() {
    // Match at position 0 of a multi-chunk sequence.
    // Verify it survives window drain in subsequent chunks.
    let mut seq = vec![b'A'; 262200];
    seq[0] = b'C';
    seq[1] = b'C';
    seq[2] = b'G';
    seq[3] = b'G';

    let out = boundary_grep(&seq, "-2", &["-G", "/CCGG/g"]);
    let rows = parse_csv_rows(&out);
    assert_eq!(rows.len(), 1, "expected match at pos 0, got: {}", out);
    assert_eq!(rows[0][1], "0");
    assert_eq!(rows[0][3], "CCGG");
}
