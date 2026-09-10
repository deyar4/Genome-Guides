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

use std::env;

fn sanitize_enabled() -> bool {
    env::var("HECATE_SANITIZE").is_ok()
}

fn apply_sanitize_flags(build: &mut cc::Build) {
    if sanitize_enabled() {
        build.flag_if_supported("-fsanitize=address,undefined");
        build.flag_if_supported("-fno-omit-frame-pointer");
    }
}

fn build_hdiff() {
    let mut cpp_build = cc::Build::new();
    cpp_build.cpp(true);
    cpp_build.include("vendor");
    cpp_build.include("vendor/hdiff");
    cpp_build.include("vendor/hpatch");
    cpp_build.define("HECATE_OPENMP", None);
    cpp_build.flag_if_supported("-std=c++11");

    // Use the OpenMP flag(s) provided by openmp-sys.
    if let Ok(omp_flag) = env::var("DEP_OPENMP_FLAG") {
        for f in omp_flag.split(' ') {
            cpp_build.flag(f);
        }
    }

    // Add OpenMP include paths (needed for Homebrew libomp on macOS).
    if let Some(include) = env::var_os("DEP_OPENMP_INCLUDE") {
        for i in env::split_paths(&include) {
            cpp_build.include(i);
        }
    }

    cpp_build.flag_if_supported("-w");
    apply_sanitize_flags(&mut cpp_build);

    let cpp_sources = [
        "vendor/hdiff/adler_roll.cpp",
        "vendor/hdiff/bytes_rle.cpp",
        "vendor/hdiff/compress_detect.cpp",
        "vendor/hdiff/diff.cpp",
        "vendor/hdiff/digest_matcher.cpp",
        "vendor/hdiff/match_block.cpp",
        "vendor/hdiff/parallel_channel.cpp",
        "vendor/hdiff/parallel_import.cpp",
        "vendor/hdiff/stream_serialize.cpp",
        "vendor/hdiff/suffix_string.cpp",
        "vendor/hdiff/hdiff_ffi.cpp",
        "vendor/hpatch/patch.cpp",
    ];

    for file in cpp_sources {
        cpp_build.file(file);
    }

    cpp_build.compile("hdiff");

    // Re-emit OpenMP link instructions for picky linkers that care about
    // library ordering (e.g. missing libgomp.so errors).
    if let Some(link) = env::var_os("DEP_OPENMP_CARGO_LINK_INSTRUCTIONS") {
        for i in env::split_paths(&link) {
            println!("cargo:{}", i.display());
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=vendor/hdiff");
    println!("cargo:rerun-if-changed=vendor/hpatch");
    println!("cargo:rerun-if-changed=vendor/config.h");

    build_hdiff();
}
