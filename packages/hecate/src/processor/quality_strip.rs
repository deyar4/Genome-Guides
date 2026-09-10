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

use crate::pipeline::handler::{Handler, RecordInfo};
use std::io;

/// Suppresses quality events, converting FASTQ to FASTA semantics.
pub struct QualityStripper<'a> {
    downstream: Box<dyn Handler + 'a>,
}

impl<'a> QualityStripper<'a> {
    pub fn new(downstream: Box<dyn Handler + 'a>) -> Self {
        Self { downstream }
    }
}

impl Handler for QualityStripper<'_> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.downstream.begin_record(header)
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.downstream.record_meta(info)
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.downstream.nuc_chunk(data)
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.downstream.case_chunk(data)
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.downstream.extra(position, ch)
    }

    fn quality_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(()) // suppressed
    }

    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        let mut info = info.clone();
        info.quality_bytes = 0;
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

    /// Test handler that records all events for verification.
    struct RecorderHandler {
        events: Rc<RefCell<Vec<String>>>,
    }

    impl RecorderHandler {
        fn new(events: Rc<RefCell<Vec<String>>>) -> Self {
            Self { events }
        }
    }

    impl Handler for RecorderHandler {
        fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
            self.events
                .borrow_mut()
                .push(format!("begin:{}", std::str::from_utf8(header).unwrap()));
            Ok(())
        }
        fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.events.borrow_mut().push(format!("nuc:{}", data.len()));
            Ok(())
        }
        fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.events
                .borrow_mut()
                .push(format!("case:{}", data.len()));
            Ok(())
        }
        fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
            self.events
                .borrow_mut()
                .push(format!("extra:{}:{}", position, ch as char));
            Ok(())
        }
        fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.events
                .borrow_mut()
                .push(format!("quality:{}", data.len()));
            Ok(())
        }
        fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
            self.events.borrow_mut().push(format!(
                "end:bases={},quality={}",
                info.base_count, info.quality_bytes
            ));
            Ok(())
        }
        fn eof(&mut self) -> io::Result<()> {
            self.events.borrow_mut().push("eof".to_string());
            Ok(())
        }
    }

    #[test]
    fn test_quality_stripper() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let recorder = Box::new(RecorderHandler::new(events.clone()));
        let mut stripper = QualityStripper::new(recorder);

        stripper.begin_record(b"read1").unwrap();
        stripper.case_chunk(&[3, 2]).unwrap();
        stripper.nuc_chunk(&[0xE4]).unwrap();
        stripper.quality_chunk(b"IIII").unwrap(); // should be suppressed
        stripper
            .end_record(&RecordInfo {
                base_count: 4,
                packed_bytes: 1,
                extra_count: 0,
                case_bytes: 2,
                quality_bytes: 4,
                nuc_skip: 0,
            })
            .unwrap();
        stripper.eof().unwrap();

        let evts = events.borrow();
        assert!(evts.contains(&"begin:read1".to_string()));
        assert!(evts.contains(&"case:2".to_string()));
        assert!(evts.contains(&"nuc:1".to_string()));
        assert!(!evts.iter().any(|e| e.starts_with("quality:")));
        assert!(evts.contains(&"end:bases=4,quality=0".to_string()));
        assert!(evts.contains(&"eof".to_string()));
    }
}
