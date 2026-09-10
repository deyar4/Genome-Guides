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

use std::ffi::c_void;

#[repr(C)]
struct HdiffBuffer {
    data: *mut u8,
    len: usize,
    handle: *mut c_void,
    status: i32,
}

type HdiffReadCallback = unsafe extern "C" fn(
    user_data: *const c_void,
    read_from_pos: u64,
    out_data: *mut u8,
    out_data_end: *mut u8,
) -> i32;

type HdiffWriteCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    write_to_pos: u64,
    data: *const u8,
    data_end: *const u8,
) -> i32;

#[repr(C)]
struct HdiffIOCallbacks {
    read_user: *const c_void,
    read_fn: Option<HdiffReadCallback>,
    write_user: *mut c_void,
    write_fn: Option<HdiffWriteCallback>,
}

unsafe extern "C" {
    fn hdiff_create_single_compressed_diff_block(
        new_data: *mut u8,
        new_len: usize,
        old_data: *mut u8,
        old_len: usize,
        patch_step_mem_size: usize,
        is_use_big_cache_match: i32,
        match_block_size: usize,
        thread_num: usize,
        min_single_match_score: i32,
    ) -> HdiffBuffer;

    fn hdiff_patch_single_compressed_diff(
        old_data: *const u8,
        old_len: usize,
        diff_data: *const u8,
        diff_len: usize,
    ) -> HdiffBuffer;

    fn hdiff_patch_single_compressed_diff_callbacks(
        callbacks: *const HdiffIOCallbacks,
        old_size: u64,
        diff_data: *const u8,
        diff_len: usize,
    ) -> i32;

    fn hdiff_free_buffer(handle: *mut c_void);
}

pub const HDIFF_DEFAULT_PATCH_STEP_MEM_SIZE: usize = 262144;
pub const HDIFF_DEFAULT_MATCH_BLOCK_SIZE: usize = 4096;
pub const HDIFF_DEFAULT_MIN_MATCH_SCORE: i32 = 9;

#[derive(Debug, Clone)]
pub struct HdiffOptions {
    pub patch_step_mem_size: usize,
    pub use_big_cache_match: bool,
    pub match_block_size: usize,
    pub thread_num: usize,
    pub min_single_match_score: i32,
}

impl Default for HdiffOptions {
    fn default() -> Self {
        Self {
            patch_step_mem_size: HDIFF_DEFAULT_PATCH_STEP_MEM_SIZE,
            use_big_cache_match: false,
            match_block_size: HDIFF_DEFAULT_MATCH_BLOCK_SIZE,
            thread_num: 1,
            min_single_match_score: HDIFF_DEFAULT_MIN_MATCH_SCORE,
        }
    }
}

#[derive(Debug)]
pub enum HdiffError {
    CreationFailed,
    PatchFailed,
}

pub struct HdiffOwnedBuffer {
    data: *mut u8,
    len: usize,
    handle: *mut c_void,
}

impl HdiffOwnedBuffer {
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.len) }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl Drop for HdiffOwnedBuffer {
    fn drop(&mut self) {
        unsafe { hdiff_free_buffer(self.handle) };
    }
}

pub fn create_single_compressed_diff_block(
    new_data: &mut [u8],
    old_data: &mut [u8],
    options: HdiffOptions,
) -> Result<HdiffOwnedBuffer, HdiffError> {
    let result = unsafe {
        hdiff_create_single_compressed_diff_block(
            new_data.as_mut_ptr(),
            new_data.len(),
            old_data.as_mut_ptr(),
            old_data.len(),
            options.patch_step_mem_size,
            if options.use_big_cache_match { 1 } else { 0 },
            options.match_block_size,
            options.thread_num,
            options.min_single_match_score,
        )
    };

    if result.status != 0 || result.data.is_null() || result.handle.is_null() {
        if !result.handle.is_null() {
            unsafe { hdiff_free_buffer(result.handle) };
        }
        return Err(HdiffError::CreationFailed);
    }

    Ok(HdiffOwnedBuffer {
        data: result.data,
        len: result.len,
        handle: result.handle,
    })
}

pub fn patch_single_compressed_diff(
    old_data: &[u8],
    diff_data: &[u8],
) -> Result<HdiffOwnedBuffer, HdiffError> {
    let result = unsafe {
        hdiff_patch_single_compressed_diff(
            old_data.as_ptr(),
            old_data.len(),
            diff_data.as_ptr(),
            diff_data.len(),
        )
    };

    if result.status != 0 || result.data.is_null() || result.handle.is_null() {
        if !result.handle.is_null() {
            unsafe { hdiff_free_buffer(result.handle) };
        }
        return Err(HdiffError::PatchFailed);
    }

    Ok(HdiffOwnedBuffer {
        data: result.data,
        len: result.len,
        handle: result.handle,
    })
}

pub fn patch_single_compressed_diff_with_callbacks<
    R: FnMut(u64, &mut [u8]) -> bool,
    W: FnMut(u64, &[u8]) -> bool,
>(
    old_size: u64,
    diff_data: &[u8],
    reader: R,
    writer: W,
) -> Result<(), HdiffError> {
    struct State<R, W> {
        reader: R,
        writer: W,
    }

    extern "C" fn read_trampoline<R, W>(
        user_data: *const c_void,
        read_from_pos: u64,
        out_data: *mut u8,
        out_data_end: *mut u8,
    ) -> i32
    where
        R: FnMut(u64, &mut [u8]) -> bool,
        W: FnMut(u64, &[u8]) -> bool,
    {
        let state = unsafe { &mut *(user_data as *mut State<R, W>) };
        let len = (out_data_end as usize).saturating_sub(out_data as usize);
        let buf = unsafe { std::slice::from_raw_parts_mut(out_data, len) };
        if (state.reader)(read_from_pos, buf) {
            1
        } else {
            0
        }
    }

    extern "C" fn write_trampoline<R, W>(
        user_data: *mut c_void,
        write_to_pos: u64,
        data: *const u8,
        data_end: *const u8,
    ) -> i32
    where
        R: FnMut(u64, &mut [u8]) -> bool,
        W: FnMut(u64, &[u8]) -> bool,
    {
        let state = unsafe { &mut *(user_data as *mut State<R, W>) };
        let len = (data_end as usize).saturating_sub(data as usize);
        let buf = unsafe { std::slice::from_raw_parts(data, len) };
        if (state.writer)(write_to_pos, buf) {
            1
        } else {
            0
        }
    }

    let mut state = State { reader, writer };
    let callbacks = HdiffIOCallbacks {
        read_user: &mut state as *mut _ as *const c_void,
        read_fn: Some(read_trampoline::<R, W>),
        write_user: &mut state as *mut _ as *mut c_void,
        write_fn: Some(write_trampoline::<R, W>),
    };

    let status = unsafe {
        hdiff_patch_single_compressed_diff_callbacks(
            &callbacks,
            old_size,
            diff_data.as_ptr(),
            diff_data.len(),
        )
    };

    if status == 0 {
        Ok(())
    } else {
        Err(HdiffError::PatchFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::patch_single_compressed_diff_with_callbacks;
    use super::{create_single_compressed_diff_block, patch_single_compressed_diff, HdiffOptions};

    #[test]
    fn creates_diff() {
        let mut new_data = b"new content".to_vec();
        let mut old_data = b"old content".to_vec();
        let diff = create_single_compressed_diff_block(
            &mut new_data,
            &mut old_data,
            HdiffOptions::default(),
        )
        .expect("hdiff should succeed");
        assert!(!diff.as_slice().is_empty());
    }

    #[test]
    fn roundtrip_patch() {
        let mut new_data = b"some updated content".to_vec();
        let mut old_data = b"some outdated content".to_vec();

        let diff = create_single_compressed_diff_block(
            &mut new_data,
            &mut old_data,
            HdiffOptions::default(),
        )
        .expect("create diff should succeed");

        let patched =
            patch_single_compressed_diff(&old_data, diff.as_slice()).expect("patch should succeed");

        assert_eq!(patched.as_slice(), new_data.as_slice());
    }

    #[test]
    fn roundtrip_patch_callbacks() {
        let mut new_data = b"another updated content".to_vec();
        let mut old_data = b"another outdated content".to_vec();

        let diff = create_single_compressed_diff_block(
            &mut new_data,
            &mut old_data,
            HdiffOptions::default(),
        )
        .expect("create diff should succeed");

        // emulate random-read + sequential-write callbacks
        let mut writes: Vec<u8> = Vec::new();
        patch_single_compressed_diff_with_callbacks(
            old_data.len() as u64,
            diff.as_slice(),
            |pos, out| {
                let start = pos as usize;
                let end = start + out.len();
                if end > old_data.len() {
                    return false;
                }
                out.copy_from_slice(&old_data[start..end]);
                true
            },
            |pos, data| {
                // sequential writes expected
                let expected_pos = writes.len() as u64;
                if pos != expected_pos {
                    return false;
                }
                writes.extend_from_slice(data);
                true
            },
        )
        .expect("patch with callbacks should succeed");

        assert_eq!(writes.as_slice(), new_data);
    }

    #[test]
    fn empty_diff_creates_but_patch_fails() {
        // hdiff C library cannot patch empty data -- referential code
        // handles this by skipping hdiff for empty streams.
        let mut new_data: Vec<u8> = vec![];
        let mut old_data: Vec<u8> = vec![];
        let diff = create_single_compressed_diff_block(
            &mut new_data,
            &mut old_data,
            HdiffOptions::default(),
        )
        .expect("empty diff creation should succeed");
        assert!(!diff.as_slice().is_empty());
        assert!(patch_single_compressed_diff(&old_data, diff.as_slice()).is_err());
    }
}
