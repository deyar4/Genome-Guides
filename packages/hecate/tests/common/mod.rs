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

use std::process::Command;

pub fn hecate() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hecate"))
}

pub fn data_path(name: &str) -> String {
    format!("{}/tests/data/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Normalize a FASTA string: unwrap sequence lines (removes line breaks within
/// sequences so each sequence is on a single line).
pub fn normalize_fasta(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        if line.starts_with('>') {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            out.push('\n');
        } else {
            out.push_str(line);
        }
    }
    out.push('\n');
    out
}

/// Extract the raw sequence bases from a FASTA file (strip headers, unwrap).
pub fn extract_bases(fasta: &str) -> String {
    let mut bases = String::new();
    for line in fasta.lines() {
        if !line.starts_with('>') {
            bases.push_str(line);
        }
    }
    bases
}

/// Extract all sequence bases preserving case (no uppercasing).
pub fn extract_bases_preserving_case(fasta: &str) -> String {
    let mut bases = String::new();
    for line in fasta.lines() {
        if !line.starts_with('>') {
            bases.push_str(line);
        }
    }
    bases
}
