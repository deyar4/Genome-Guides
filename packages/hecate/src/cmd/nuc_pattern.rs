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

use memchr::memmem;
use std::io;

/// Sentinel value used for single-base wildcards (`#` in pattern syntax).
const WILDCARD: u8 = 0xFF;

/// A parsed nucleotide search pattern, pre-compiled for fast matching.
///
/// Internally dispatches to SIMD-accelerated `memchr::memmem` for literal
/// patterns, or uses a prefilter-and-verify strategy when wildcards are
/// present.
pub struct NucPattern {
    /// Each atom is either an uppercase ASCII literal or `WILDCARD`.
    atoms: Vec<u8>,
    /// Whether matching is case-insensitive.
    pub case_insensitive: bool,
    /// Whether to report all matches (true) or just the first per sequence.
    pub global: bool,

    // --- Pre-computed acceleration ---
    has_wildcards: bool,
    /// Longest contiguous run of non-wildcard bytes in the pattern.
    /// Used as the prefilter anchor when wildcards are present.
    anchor_offset: usize,
    anchor: Vec<u8>,

    /// Cached SIMD-accelerated finder for the full literal pattern
    /// (used when no wildcards are present).
    literal_finder: Option<memmem::Finder<'static>>,
    /// Cached SIMD-accelerated finder for the anchor substring
    /// (used as prefilter when wildcards are present).
    anchor_finder: Option<memmem::Finder<'static>>,
}

/// A single match result within a sequence.
pub struct PatternMatch {
    /// 0-based position within the sequence where the match starts.
    pub pos: usize,
    /// The matched bytes (from the haystack, preserving original case).
    pub matched: Vec<u8>,
}

/// Parse a `/body/flags` pattern string into a `NucPattern`.
///
/// - `#` in the body becomes a single-base wildcard.
/// - All other characters are stored as uppercase literals.
/// - Flags: `i` = case-insensitive, `g` = global (all matches).
pub fn parse_nuc_pattern(s: &str) -> io::Result<NucPattern> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] != b'/' {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pattern must start with '/'",
        ));
    }
    // Find the closing slash
    let close = match bytes[1..].iter().position(|&b| b == b'/') {
        Some(p) => p + 1,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "pattern must end with '/' (possibly followed by flags)",
            ));
        }
    };
    let body = &bytes[1..close];
    let flags = &bytes[close + 1..];

    if body.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pattern body is empty",
        ));
    }

    let mut case_insensitive = false;
    let mut global = false;
    for &f in flags {
        match f {
            b'i' => case_insensitive = true,
            b'g' => global = true,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown pattern flag '{}'", f as char),
                ));
            }
        }
    }

    let atoms: Vec<u8> = body
        .iter()
        .map(|&b| {
            if b == b'#' {
                WILDCARD
            } else {
                b.to_ascii_uppercase()
            }
        })
        .collect();

    let has_wildcards = atoms.iter().any(|&a| a == WILDCARD);
    let (anchor_offset, anchor) = longest_literal_run(&atoms);

    let literal_finder = if !has_wildcards {
        Some(memmem::Finder::new(&atoms).into_owned())
    } else {
        None
    };
    let anchor_finder = if has_wildcards && !anchor.is_empty() {
        Some(memmem::Finder::new(&anchor).into_owned())
    } else {
        None
    };

    Ok(NucPattern {
        atoms,
        case_insensitive,
        global,
        has_wildcards,
        anchor_offset,
        anchor,
        literal_finder,
        anchor_finder,
    })
}

/// Find the longest contiguous run of non-WILDCARD bytes.
/// Returns (offset, bytes).
fn longest_literal_run(atoms: &[u8]) -> (usize, Vec<u8>) {
    let mut best_start = 0;
    let mut best_len = 0usize;
    let mut cur_start = 0;
    let mut cur_len = 0usize;

    for (i, &a) in atoms.iter().enumerate() {
        if a != WILDCARD {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_start = cur_start;
                best_len = cur_len;
            }
        } else {
            cur_len = 0;
        }
    }

    let anchor = atoms[best_start..best_start + best_len].to_vec();
    (best_start, anchor)
}

impl NucPattern {
    /// Return the pattern length in bases.
    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    /// Search `haystack` for occurrences of this pattern.
    ///
    /// Returns all matches if `global` is set, otherwise at most one.
    /// Uses `memchr::memmem` for SIMD-accelerated literal search when
    /// possible.
    pub fn find_in(&self, haystack: &[u8]) -> Vec<PatternMatch> {
        let pat_len = self.atoms.len();
        if pat_len == 0 || haystack.len() < pat_len {
            return Vec::new();
        }

        if self.case_insensitive {
            let mut upper = haystack.to_vec();
            upper.make_ascii_uppercase();
            if !self.has_wildcards {
                self.find_literal(&upper, haystack)
            } else if !self.anchor.is_empty() {
                self.find_prefiltered(&upper, haystack)
            } else {
                self.find_all_wild(haystack)
            }
        } else if !self.has_wildcards {
            self.find_literal(haystack, haystack)
        } else if !self.anchor.is_empty() {
            self.find_prefiltered(haystack, haystack)
        } else {
            self.find_all_wild(haystack)
        }
    }

    /// Pure-literal search using `memchr::memmem`.
    ///
    /// `search_buf` is searched (may be uppercased); matched bytes are
    /// taken from `original` to preserve the caller's case.
    fn find_literal(&self, search_buf: &[u8], original: &[u8]) -> Vec<PatternMatch> {
        let finder = self.literal_finder.as_ref().unwrap();
        let pat_len = self.atoms.len();
        let mut results = Vec::new();
        let mut pos = 0;

        while pos + pat_len <= search_buf.len() {
            match finder.find(&search_buf[pos..]) {
                Some(offset) => {
                    let abs = pos + offset;
                    results.push(PatternMatch {
                        pos: abs,
                        matched: original[abs..abs + pat_len].to_vec(),
                    });
                    if !self.global {
                        break;
                    }
                    pos = abs + 1;
                }
                None => break,
            }
        }

        results
    }

    /// Prefilter-and-verify search for patterns with wildcards.
    ///
    /// Uses `memchr::memmem` to find candidates for the longest literal
    /// run in the pattern, then verifies the remaining (wildcard)
    /// positions with a cheap scalar check.
    fn find_prefiltered(&self, search_buf: &[u8], original: &[u8]) -> Vec<PatternMatch> {
        let finder = self.anchor_finder.as_ref().unwrap();
        let pat_len = self.atoms.len();
        let anchor_off = self.anchor_offset;
        let anchor_len = self.anchor.len();
        let mut results = Vec::new();
        let mut pos = 0;

        while pos + pat_len <= search_buf.len() {
            // The anchor starts at `candidate + anchor_off` in the haystack,
            // so we search from `pos + anchor_off` and need enough room for
            // the tail after the anchor.
            let search_start = pos + anchor_off;
            let tail_after_anchor = pat_len - anchor_off - anchor_len;
            if search_start + anchor_len + tail_after_anchor > search_buf.len() {
                break;
            }
            let search_end = search_buf.len() - tail_after_anchor;

            match finder.find(&search_buf[search_start..search_end]) {
                Some(offset) => {
                    let candidate = search_start + offset - anchor_off;
                    if candidate >= pos
                        && candidate + pat_len <= search_buf.len()
                        && self.verify_non_anchor(search_buf, candidate)
                    {
                        results.push(PatternMatch {
                            pos: candidate,
                            matched: original[candidate..candidate + pat_len].to_vec(),
                        });
                        if !self.global {
                            break;
                        }
                    }
                    // Advance past this candidate (whether it verified or not)
                    pos = candidate + 1;
                }
                None => break,
            }
        }

        results
    }

    /// Verify non-anchor positions (wildcards and any literals outside the
    /// anchor run) at a candidate offset.  The anchor itself was already
    /// confirmed by `memmem`.
    #[inline]
    fn verify_non_anchor(&self, buf: &[u8], offset: usize) -> bool {
        let anchor_end = self.anchor_offset + self.anchor.len();
        for (j, &atom) in self.atoms.iter().enumerate() {
            // Skip wildcards and the anchor run (already matched by memmem)
            if atom == WILDCARD || (j >= self.anchor_offset && j < anchor_end) {
                continue;
            }
            if buf[offset + j] != atom {
                return false;
            }
        }
        true
    }

    /// Degenerate case: pattern is all wildcards.  Every position of the
    /// right length is a match.
    fn find_all_wild(&self, haystack: &[u8]) -> Vec<PatternMatch> {
        let pat_len = self.atoms.len();
        let limit = haystack.len() - pat_len + 1;
        let mut results = Vec::new();

        for i in 0..limit {
            results.push(PatternMatch {
                pos: i,
                matched: haystack[i..i + pat_len].to_vec(),
            });
            if !self.global {
                break;
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let p = parse_nuc_pattern("/ACGT/").unwrap();
        assert_eq!(p.atoms, vec![b'A', b'C', b'G', b'T']);
        assert!(!p.case_insensitive);
        assert!(!p.global);
        assert!(!p.has_wildcards);
    }

    #[test]
    fn test_parse_flags() {
        let p = parse_nuc_pattern("/ACGT/ig").unwrap();
        assert!(p.case_insensitive);
        assert!(p.global);
    }

    #[test]
    fn test_parse_wildcard() {
        let p = parse_nuc_pattern("/A#G/").unwrap();
        assert_eq!(p.atoms, vec![b'A', WILDCARD, b'G']);
        assert!(p.has_wildcards);
        // Longest literal run: either "A" (len 1) or "G" (len 1).
        // Implementation picks the first one of max length.
        assert_eq!(p.anchor.len(), 1);
    }

    #[test]
    fn test_parse_errors() {
        assert!(parse_nuc_pattern("ACGT").is_err());
        assert!(parse_nuc_pattern("/ACGT").is_err());
        assert!(parse_nuc_pattern("//").is_err());
        assert!(parse_nuc_pattern("/ACGT/x").is_err());
    }

    #[test]
    fn test_longest_literal_run() {
        // "AAG###GGG" -> anchor is "GGG" at offset 6
        let atoms = vec![
            b'A', b'A', b'G', WILDCARD, WILDCARD, WILDCARD, b'G', b'G', b'G',
        ];
        let (off, anchor) = longest_literal_run(&atoms);
        assert_eq!(off, 0); // "AAG" at offset 0 has length 3, same as "GGG" at 6; first wins
        assert_eq!(anchor, b"AAG");

        // "A#BCDE" -> anchor is "BCDE" at offset 2
        let atoms2 = vec![b'A', WILDCARD, b'B', b'C', b'D', b'E'];
        let (off2, anchor2) = longest_literal_run(&atoms2);
        assert_eq!(off2, 2);
        assert_eq!(anchor2, b"BCDE");
    }

    #[test]
    fn test_find_exact() {
        let p = parse_nuc_pattern("/ACGT/").unwrap();
        let matches = p.find_in(b"XXXACGTYYY");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 3);
        assert_eq!(matches[0].matched, b"ACGT");
    }

    #[test]
    fn test_find_case_insensitive() {
        let p = parse_nuc_pattern("/ACGT/i").unwrap();
        let matches = p.find_in(b"xxxacgtyyy");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 3);
        assert_eq!(matches[0].matched, b"acgt");
    }

    #[test]
    fn test_find_global() {
        let p = parse_nuc_pattern("/AA/g").unwrap();
        let matches = p.find_in(b"AAAAA");
        assert_eq!(matches.len(), 4); // overlapping: 0,1,2,3
    }

    #[test]
    fn test_find_no_match() {
        let p = parse_nuc_pattern("/GGGG/").unwrap();
        let matches = p.find_in(b"ACGTACGT");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_first_only() {
        let p = parse_nuc_pattern("/ACGT/").unwrap();
        let matches = p.find_in(b"ACGTACGT");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 0);
    }

    #[test]
    fn test_wildcard_match() {
        let p = parse_nuc_pattern("/A#G/").unwrap();
        let matches = p.find_in(b"XACGX");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 1);
        assert_eq!(matches[0].matched, b"ACG");
    }

    #[test]
    fn test_wildcard_global_overlapping() {
        // Pattern "A#A" with wildcards, global, should find overlapping matches
        let p = parse_nuc_pattern("/A#A/g").unwrap();
        let matches = p.find_in(b"AXAXAXA");
        // Matches at 0 (AXA), 2 (AXA), 4 (AXA)
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].pos, 0);
        assert_eq!(matches[1].pos, 2);
        assert_eq!(matches[2].pos, 4);
    }

    #[test]
    fn test_wildcard_case_insensitive() {
        let p = parse_nuc_pattern("/A#G/ig").unwrap();
        let matches = p.find_in(b"xacgx");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 1);
        assert_eq!(matches[0].matched, b"acg"); // preserves original case
    }

    #[test]
    fn test_all_wildcards() {
        let p = parse_nuc_pattern("/##/g").unwrap();
        let matches = p.find_in(b"ACGT");
        assert_eq!(matches.len(), 3); // 0,1,2
    }

    #[test]
    fn test_prefilter_multi_anchor() {
        // "AAG###GGG" -- two literal runs of length 3
        let p = parse_nuc_pattern("/AAG###GGG/g").unwrap();
        let matches = p.find_in(b"AAGXXXGGG");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 0);
        assert_eq!(matches[0].matched, b"AAGXXXGGG");
    }

    #[test]
    fn test_prefilter_no_false_positive() {
        // Anchor "GGG" found but surrounding wildcards don't extend far enough
        let p = parse_nuc_pattern("/##GGG##/g").unwrap();
        let matches = p.find_in(b"XGGGX");
        // Pattern is 7 chars but haystack is only 5
        assert!(matches.is_empty());
    }

    #[test]
    fn test_literal_at_end_of_haystack() {
        let p = parse_nuc_pattern("/ACGT/g").unwrap();
        let matches = p.find_in(b"ACGT");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pos, 0);
    }
}
