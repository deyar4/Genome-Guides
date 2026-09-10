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

use crate::container::AlphabetMode;
use crate::pipeline::handler::{Handler, RecordInfo};
use std::io;

/// Removes case information. For 8-bit mode, uppercases nucleotide bytes.
/// For 2-bit/4-bit, packed codes are case-independent so no nuc transform needed.
pub struct CaseStripper<'a> {
    downstream: Box<dyn Handler + 'a>,
    eight_bit: bool,
    buf: Vec<u8>,
}

impl<'a> CaseStripper<'a> {
    pub fn new(downstream: Box<dyn Handler + 'a>, alphabet: AlphabetMode) -> Self {
        Self {
            downstream,
            eight_bit: alphabet == AlphabetMode::EightBit,
            buf: Vec::new(),
        }
    }
}

impl Handler for CaseStripper<'_> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.downstream.begin_record(header)
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.downstream.record_meta(info)
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        if self.eight_bit {
            self.buf.clear();
            self.buf.reserve(data.len());
            for &b in data {
                self.buf.push(b.to_ascii_uppercase());
            }
            self.downstream.nuc_chunk(&self.buf)
        } else {
            self.downstream.nuc_chunk(data)
        }
    }

    fn case_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(()) // suppressed
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.downstream.extra(position, ch)
    }

    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.downstream.quality_chunk(data)
    }

    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        let mut info = info.clone();
        info.case_bytes = 0;
        self.downstream.end_record(&info)
    }

    fn eof(&mut self) -> io::Result<()> {
        self.downstream.eof()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct NucRecorder {
        nuc_data: Rc<RefCell<Vec<u8>>>,
        case_data: Rc<RefCell<Vec<u8>>>,
        end_case_bytes: Rc<RefCell<u32>>,
    }

    impl NucRecorder {
        fn new(
            nuc_data: Rc<RefCell<Vec<u8>>>,
            case_data: Rc<RefCell<Vec<u8>>>,
            end_case_bytes: Rc<RefCell<u32>>,
        ) -> Self {
            Self {
                nuc_data,
                case_data,
                end_case_bytes,
            }
        }
    }

    impl Handler for NucRecorder {
        fn begin_record(&mut self, _header: &[u8]) -> io::Result<()> {
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
        fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
            *self.end_case_bytes.borrow_mut() = info.case_bytes;
            Ok(())
        }
        fn eof(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_case_stripper_8bit() {
        let nuc_data = Rc::new(RefCell::new(Vec::new()));
        let case_data = Rc::new(RefCell::new(Vec::new()));
        let end_case_bytes = Rc::new(RefCell::new(0u32));
        let recorder = Box::new(NucRecorder::new(
            nuc_data.clone(),
            case_data.clone(),
            end_case_bytes.clone(),
        ));
        let mut stripper = CaseStripper::new(recorder, AlphabetMode::EightBit);

        stripper.begin_record(b"seq1").unwrap();
        stripper.case_chunk(&[3, 2]).unwrap(); // should be suppressed
        stripper.nuc_chunk(b"AaGgTt").unwrap(); // should be uppercased
        stripper
            .end_record(&RecordInfo {
                base_count: 6,
                packed_bytes: 6,
                extra_count: 0,
                case_bytes: 2,
                quality_bytes: 0,
                nuc_skip: 0,
            })
            .unwrap();

        assert_eq!(&*nuc_data.borrow(), b"AAGGTT");
        assert!(case_data.borrow().is_empty());
        assert_eq!(*end_case_bytes.borrow(), 0);
    }

    #[test]
    fn test_case_stripper_2bit_passthrough() {
        let nuc_data = Rc::new(RefCell::new(Vec::new()));
        let case_data = Rc::new(RefCell::new(Vec::new()));
        let end_case_bytes = Rc::new(RefCell::new(0u32));
        let recorder = Box::new(NucRecorder::new(
            nuc_data.clone(),
            case_data.clone(),
            end_case_bytes.clone(),
        ));
        let mut stripper = CaseStripper::new(recorder, AlphabetMode::TwoBit);

        stripper.begin_record(b"seq1").unwrap();
        stripper.case_chunk(&[4]).unwrap(); // should be suppressed
        stripper.nuc_chunk(&[0xE4]).unwrap(); // packed data, should pass unchanged
        stripper
            .end_record(&RecordInfo {
                base_count: 4,
                packed_bytes: 1,
                extra_count: 0,
                case_bytes: 1,
                quality_bytes: 0,
                nuc_skip: 0,
            })
            .unwrap();

        assert_eq!(&*nuc_data.borrow(), &[0xE4]);
        assert!(case_data.borrow().is_empty());
        assert_eq!(*end_case_bytes.borrow(), 0);
    }
}
