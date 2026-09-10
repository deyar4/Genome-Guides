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

extern crate openmp_sys;

pub mod codec;
pub mod compress;
pub mod container;
pub mod hdiff;
pub mod index;
pub mod markov;
pub mod mux;
pub mod pipeline;
pub mod processor;
pub mod referential;
pub mod stream;
pub mod util;

/// Mutex to serialize tests that allocate BlockCodec (~2.3 GiB each).
/// Without this, parallel test threads OOM the machine.
#[cfg(test)]
pub(crate) static HEAVY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
