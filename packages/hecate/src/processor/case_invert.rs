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

use crate::container::{AlphabetMode, CaseMode};
use crate::pipeline::handler::{Handler, RecordInfo};
use std::io;

/// Inverts all case. In RLE mode, prepends a zero-length run to flip phase.
/// In 8-bit joint mode, XORs 0x20 on alphabetic nucleotide bytes.
pub struct CaseInverter<'a> {
    downstream: Box<dyn Handler + 'a>,
    first_case_chunk: bool,
    joint_mode: bool,
    buf: Vec<u8>,
}

impl<'a> CaseInverter<'a> {
    pub fn new(
        downstream: Box<dyn Handler + 'a>,
        alphabet: AlphabetMode,
        case_mode: CaseMode,
    ) -> Self {
        Self {
            downstream,
            first_case_chunk: true,
            joint_mode: alphabet == AlphabetMode::EightBit && case_mode == CaseMode::Joint,
            buf: Vec::new(),
        }
    }
}

impl Handler for CaseInverter<'_> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.first_case_chunk = true;
        self.downstream.begin_record(header)
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.downstream.record_meta(info)
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        if self.joint_mode {
            self.buf.clear();
            self.buf.reserve(data.len());
            for &b in data {
                if b.is_ascii_alphabetic() {
                    self.buf.push(b ^ 0x20);
                } else {
                    self.buf.push(b);
                }
            }
            self.downstream.nuc_chunk(&self.buf)
        } else {
            self.downstream.nuc_chunk(data)
        }
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        if self.first_case_chunk {
            self.downstream.case_chunk(&[0u8])?;
            self.first_case_chunk = false;
        }
        self.downstream.case_chunk(data)
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.downstream.extra(position, ch)
    }

    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.downstream.quality_chunk(data)
    }

    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        if !self.first_case_chunk {
            // We prepended a zero-length run byte; update case_bytes so
            // binary sinks (HecateWriterSink) record the correct count.
            let mut info = info.clone();
            info.case_bytes += 1;
            self.downstream.end_record(&info)
        } else {
            self.downstream.end_record(info)
        }
    }

    fn eof(&mut self) -> io::Result<()> {
        self.downstream.eof()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::common::{CaseDecoder, CaseEncoder};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct CaseRecorder {
        case_data: Rc<RefCell<Vec<u8>>>,
        nuc_data: Rc<RefCell<Vec<u8>>>,
    }

    impl CaseRecorder {
        fn new(case_data: Rc<RefCell<Vec<u8>>>, nuc_data: Rc<RefCell<Vec<u8>>>) -> Self {
            Self {
                case_data,
                nuc_data,
            }
        }
    }

    impl Handler for CaseRecorder {
        fn begin_record(&mut self, _header: &[u8]) -> io::Result<()> {
            self.case_data.borrow_mut().clear();
            self.nuc_data.borrow_mut().clear();
            Ok(())
        }
        fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.nuc_data.borrow_mut().extend_from_slice(data);
            Ok(())
        }
        fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.case_data.borrow_mut().extend_from_slice(data);
            Ok(())
        }
        fn extra(&mut self, _position: u64, _ch: u8) -> io::Result<()> {
            Ok(())
        }
        fn quality_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
            Ok(())
        }
        fn end_record(&mut self, _info: &RecordInfo) -> io::Result<()> {
            Ok(())
        }
        fn eof(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_case_invert_rle() {
        // Encode "AAAaaa" -> RLE [3, 3]
        let mut enc = CaseEncoder::new();
        for _ in 0..3 {
            enc.push_bit(1);
        } // AAA
        for _ in 0..3 {
            enc.push_bit(0);
        } // aaa
        let rle = enc.finish().to_vec();
        assert_eq!(rle, vec![3, 3]);

        let case_data = Rc::new(RefCell::new(Vec::new()));
        let nuc_data = Rc::new(RefCell::new(Vec::new()));
        let recorder = Box::new(CaseRecorder::new(case_data.clone(), nuc_data.clone()));
        let mut inverter = CaseInverter::new(recorder, AlphabetMode::TwoBit, CaseMode::Include);

        inverter.begin_record(b"seq1").unwrap();
        inverter.case_chunk(&rle).unwrap();
        inverter.end_record(&RecordInfo::default()).unwrap();

        // After inversion: [0, 3, 3] means: 0 uppercase, 3 lowercase, 3 uppercase
        let inverted = case_data.borrow().clone();
        assert_eq!(inverted, vec![0, 3, 3]);

        // Decode inverted: should give "aaaAAA"
        let mut dec = CaseDecoder::new(&inverted);
        assert!(!dec.next_is_upper()); // a
        assert!(!dec.next_is_upper()); // a
        assert!(!dec.next_is_upper()); // a
        assert!(dec.next_is_upper()); // A
        assert!(dec.next_is_upper()); // A
        assert!(dec.next_is_upper()); // A
    }

    #[test]
    fn test_case_invert_joint_8bit() {
        let case_data = Rc::new(RefCell::new(Vec::new()));
        let nuc_data = Rc::new(RefCell::new(Vec::new()));
        let recorder = Box::new(CaseRecorder::new(case_data.clone(), nuc_data.clone()));
        let mut inverter = CaseInverter::new(recorder, AlphabetMode::EightBit, CaseMode::Joint);

        inverter.begin_record(b"seq1").unwrap();
        inverter.nuc_chunk(b"AaGg").unwrap();
        inverter.end_record(&RecordInfo::default()).unwrap();

        assert_eq!(&*nuc_data.borrow(), b"aAgG");
    }
}
