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

pub mod common;
pub mod fasta;
pub mod fastq;
pub mod illumina;
pub mod quality;
pub mod varint;

pub use common::{
    pack_code, CaseDecoder, CaseEncoder, StreamingCaseDecoder, CASE_BIT, FOUR_BIT_DECODE,
    FOUR_BIT_ENCODE, TWO_BIT_DECODE, TWO_BIT_ENCODE,
};
