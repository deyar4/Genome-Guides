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

pub mod bwt_codec;
pub mod rans_codec;
pub mod traits;
pub mod markov_codec;
pub mod xz_codec;
pub mod zstd_codec;

pub use bwt_codec::{BwtCompressor, BwtDecompressor};
pub use rans_codec::{RansCompressor, RansDecompressor};
pub use traits::{CompressionMethod, Compressor, Decompressor};
pub use markov_codec::{MarkovCompressor, MarkovDecompressor};
pub use xz_codec::{XzCompressor, XzDecompressor, XzParams};
pub use zstd_codec::{ZstdCompressor, ZstdDecompressor};
