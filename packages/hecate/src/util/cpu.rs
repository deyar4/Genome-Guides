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

/// Return the number of available CPU cores, suitable for parallelism.
pub fn available_cores() -> usize {
    num_cpus::get().max(1)
}

/// Read total physical memory.  Uses /proc/meminfo on Linux,
/// GlobalMemoryStatusEx on Windows.  Falls back to 4 GiB if the
/// platform query fails.
pub fn system_memory_bytes() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    let kb_str = rest.trim().trim_end_matches(" kB").trim();
                    if let Ok(kb) = kb_str.parse::<usize>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        return windows_memory().0;
    }
    4 * 1024 * 1024 * 1024 // 4 GiB fallback
}

/// Read memory currently available for allocation without swapping.
/// Uses `MemAvailable` from /proc/meminfo on Linux, `ull_avail_phys`
/// on Windows.  Falls back to `system_memory_bytes() / 2` when the
/// query fails (conservative: assumes half of RAM is in use).
pub fn available_memory_bytes() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("MemAvailable:") {
                    let kb_str = rest.trim().trim_end_matches(" kB").trim();
                    if let Ok(kb) = kb_str.parse::<usize>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        return windows_memory().1;
    }
    system_memory_bytes() / 2
}

#[cfg(target_os = "windows")]
fn windows_memory() -> (usize, usize) {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
    }
    unsafe {
        let mut status = std::mem::zeroed::<MemoryStatusEx>();
        status.dw_length = std::mem::size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(&mut status) != 0 {
            return (
                status.ull_total_phys as usize,
                status.ull_avail_phys as usize,
            );
        }
    }
    let fallback = 4 * 1024 * 1024 * 1024usize;
    (fallback, fallback / 2)
}
