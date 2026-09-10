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

use super::traits::{StreamRead, StreamWrite};
use std::io::{self, Read, Write};

/// Stdin stream using a locked handle.
pub struct StdinStream {
    inner: io::StdinLock<'static>,
}

impl StdinStream {
    pub fn new() -> Self {
        // Leak a Box<Stdin> so we get a 'static lock.
        let stdin = Box::leak(Box::new(io::stdin()));
        Self {
            inner: stdin.lock(),
        }
    }
}

impl Default for StdinStream {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamRead for StdinStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Stdout stream using a locked handle.
pub struct StdoutStream {
    inner: io::StdoutLock<'static>,
}

impl StdoutStream {
    pub fn new() -> Self {
        let stdout = Box::leak(Box::new(io::stdout()));
        Self {
            inner: stdout.lock(),
        }
    }
}

impl Default for StdoutStream {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamWrite for StdoutStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
