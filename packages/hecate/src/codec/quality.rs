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

/// Quality score binning for lossy FASTQ compression.
use std::io;

/// Specification for quality binning.
pub enum QualityBinSpec {
    /// 8-level Illumina binning scheme.
    Illumina,
    /// N uniform bins across Phred range [0, 41].
    Uniform(u8),
}

/// Build a 256-entry LUT that maps ASCII quality byte -> binned ASCII quality byte.
pub fn build_quality_lut(spec: &QualityBinSpec) -> [u8; 256] {
    let mut lut = [0u8; 256];
    // Identity for all values first
    for i in 0..256 {
        lut[i] = i as u8;
    }
    match spec {
        QualityBinSpec::Illumina => {
            // Map Phred values to bin centers, operating on ASCII (Phred + 33)
            for phred in 0u8..=41 {
                let binned = illumina_bin(phred);
                lut[(phred + 33) as usize] = binned + 33;
            }
        }
        QualityBinSpec::Uniform(n) => {
            let n = *n as u32;
            if n == 0 {
                return lut;
            }
            // Divide [0, 41] into n uniform bins
            for phred in 0u8..=41 {
                let bin_idx = (phred as u32 * n) / 42;
                let bin_start = (bin_idx * 42) / n;
                let bin_end = ((bin_idx + 1) * 42) / n;
                let center = ((bin_start + bin_end - 1) / 2) as u8;
                lut[(phred + 33) as usize] = center + 33;
            }
        }
    }
    lut
}

/// Illumina 8-level binning: Phred value -> bin center Phred value.
fn illumina_bin(phred: u8) -> u8 {
    match phred {
        0 => 0,
        1 => 1,
        2..=9 => 6,
        10..=19 => 15,
        20..=24 => 22,
        25..=29 => 27,
        30..=34 => 33,
        35..=39 => 37,
        _ => 40, // 40+
    }
}

/// Parse a quality binning spec string: "illumina" or "bin:N".
pub fn parse_quality_bin_spec(s: &str) -> io::Result<QualityBinSpec> {
    if s == "illumina" {
        return Ok(QualityBinSpec::Illumina);
    }
    if let Some(rest) = s.strip_prefix("bin:") {
        let n: u8 = rest.parse().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bad quality bin count: {}", e),
            )
        })?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "quality bin count must be > 0",
            ));
        }
        return Ok(QualityBinSpec::Uniform(n));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "unknown quality binning spec: {:?} (expected \"illumina\" or \"bin:N\")",
            s
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_illumina_bin_boundaries() {
        assert_eq!(illumina_bin(0), 0);
        assert_eq!(illumina_bin(1), 1);
        assert_eq!(illumina_bin(2), 6);
        assert_eq!(illumina_bin(9), 6);
        assert_eq!(illumina_bin(10), 15);
        assert_eq!(illumina_bin(19), 15);
        assert_eq!(illumina_bin(20), 22);
        assert_eq!(illumina_bin(24), 22);
        assert_eq!(illumina_bin(25), 27);
        assert_eq!(illumina_bin(29), 27);
        assert_eq!(illumina_bin(30), 33);
        assert_eq!(illumina_bin(34), 33);
        assert_eq!(illumina_bin(35), 37);
        assert_eq!(illumina_bin(39), 37);
        assert_eq!(illumina_bin(40), 40);
        assert_eq!(illumina_bin(41), 40);
    }

    #[test]
    fn test_illumina_lut_ascii() {
        let lut = build_quality_lut(&QualityBinSpec::Illumina);
        // Phred 0 = ASCII 33 -> binned Phred 0 = ASCII 33
        assert_eq!(lut[33], 33);
        // Phred 1 = ASCII 34 -> binned Phred 1 = ASCII 34
        assert_eq!(lut[34], 34);
        // Phred 5 = ASCII 38 -> binned Phred 6 = ASCII 39
        assert_eq!(lut[38], 39);
        // Phred 15 = ASCII 48 -> binned Phred 15 = ASCII 48
        assert_eq!(lut[48], 48);
        // Phred 20 = ASCII 53 -> binned Phred 22 = ASCII 55
        assert_eq!(lut[53], 55);
        // Phred 40 = ASCII 73 -> binned Phred 40 = ASCII 73
        assert_eq!(lut[73], 73);
    }

    #[test]
    fn test_illumina_lut_passthrough() {
        let lut = build_quality_lut(&QualityBinSpec::Illumina);
        // Values outside Phred range should pass through
        assert_eq!(lut[0], 0);
        assert_eq!(lut[32], 32);
        assert_eq!(lut[75], 75); // Phred 42 is beyond range, identity
        assert_eq!(lut[255], 255);
    }

    #[test]
    fn test_uniform_binning() {
        let lut = build_quality_lut(&QualityBinSpec::Uniform(2));
        // 2 bins: [0-20] -> center ~10, [21-41] -> center ~31
        // Just verify it doesn't panic and produces valid mappings
        for phred in 0..=41 {
            let binned = lut[(phred + 33) as usize];
            assert!(binned >= 33);
        }
    }

    #[test]
    fn test_parse_illumina_spec() {
        assert!(matches!(
            parse_quality_bin_spec("illumina").unwrap(),
            QualityBinSpec::Illumina
        ));
    }

    #[test]
    fn test_parse_bin_spec() {
        match parse_quality_bin_spec("bin:8").unwrap() {
            QualityBinSpec::Uniform(n) => assert_eq!(n, 8),
            _ => panic!("expected Uniform"),
        }
    }

    #[test]
    fn test_parse_invalid_spec() {
        assert!(parse_quality_bin_spec("foo").is_err());
        assert!(parse_quality_bin_spec("bin:0").is_err());
        assert!(parse_quality_bin_spec("bin:abc").is_err());
    }
}
