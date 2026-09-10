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

pub mod case_invert;
pub mod case_strip;
pub mod quality_strip;
pub mod slice_filter;

use crate::container::{AlphabetMode, CaseMode};
use crate::pipeline::handler::Handler;

/// Specification for a processor in the pipeline.
#[derive(Debug, Clone)]
pub enum ProcSpec {
    StripQuality,
    StripCase {
        alphabet: AlphabetMode,
    },
    InvertCase {
        alphabet: AlphabetMode,
        case_mode: CaseMode,
    },
    Slice {
        specs: Vec<slice_filter::SliceSpec>,
        alphabet: AlphabetMode,
    },
}

/// Construct the handler chain from processor specs.
/// Wraps processors inside-out: last spec = innermost wrapper.
pub fn build_handler<'a>(specs: &[ProcSpec], sink: Box<dyn Handler + 'a>) -> Box<dyn Handler + 'a> {
    let mut handler = sink;

    for spec in specs.iter().rev() {
        handler = match spec {
            ProcSpec::StripQuality => Box::new(quality_strip::QualityStripper::new(handler)),
            ProcSpec::StripCase { alphabet } => {
                Box::new(case_strip::CaseStripper::new(handler, *alphabet))
            }
            ProcSpec::InvertCase {
                alphabet,
                case_mode,
            } => Box::new(case_invert::CaseInverter::new(
                handler, *alphabet, *case_mode,
            )),
            ProcSpec::Slice { specs, alphabet } => Box::new(slice_filter::SliceFilter::new(
                handler,
                specs.clone(),
                *alphabet,
            )),
        };
    }

    handler
}
