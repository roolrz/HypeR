#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Prepare an isolated, pinned rust-src overlay; never edit rustup's sources."""
import pathlib
import shutil
import subprocess
import sys

source, destination, overlay = map(pathlib.Path, sys.argv[1:])
version = subprocess.check_output(["rustc", "--version"], text=True).split()[1]
if version != "1.97.1":
    sys.exit("HypeR std patches require rustc 1.97.1 and its matching rust-src")
shutil.copytree(source, destination)
shutil.copytree(overlay, destination, dirs_exist_ok=True)


def replace(path, old, new, count=1):
    file = destination / path
    text = file.read_text()
    if text.count(old) != count:
        sys.exit(f"Rust std patch anchor changed: {path}: {old!r}")
    file.write_text(text.replace(old, new, 1))


def select(module, exports="*"):
    branch = '    target_os = "hyper" => {\n        mod hyper;\n'
    if exports:
        branch += f"        pub use hyper::{exports};\n"
    branch += "    }\n"
    replace(f"std/src/sys/{module}/mod.rs", "cfg_select! {", "cfg_select! {\n" + branch)


replace("std/build.rs", 'if target_os == "linux"', 'if target_os == "hyper" || target_os == "linux"')
for module in ["pal", "args", "env", "stdio", "thread"]:
    select(module)
select("alloc", "")
select("pipe")
replace("std/src/sys/paths/mod.rs", "cfg_select! {", 'cfg_select! {\n    target_os = "hyper" => { mod hyper; use hyper as imp; }')
replace("std/src/sys/process/mod.rs", "cfg_select! {", 'cfg_select! {\n    target_os = "hyper" => { mod hyper; use hyper as imp; }')
replace("std/src/sys/fs/mod.rs", "cfg_select! {", 'cfg_select! {\n    target_os = "hyper" => { mod hyper; use hyper as imp; }')
replace("std/src/sys/args/mod.rs", '#[cfg(any(', '#[cfg(any(target_os = "hyper",')
replace("std/src/sys/io/error/mod.rs", '        target_os = "vexos",', '        target_os = "hyper",\n        target_os = "vexos",')
replace("std/src/sys/exit.rs", 'pub fn exit(code: i32) -> ! {\n    cfg_select! {',
        'pub fn exit(code: i32) -> ! {\n    cfg_select! {\n        target_os = "hyper" => { unsafe { crate::sys::pal::ffi::__hyper_std_exit(code) } }')

# Use the upstream atomic/futex algorithms, never the single-thread fallbacks.
for primitive in ["mutex", "condvar", "rwlock", "once", "thread_parking"]:
    replace(f"std/src/sys/sync/{primitive}/mod.rs", "    any(", '    any(target_os = "hyper",', count=3 if primitive == "thread_parking" else 2)

replace("std/src/sys/thread_local/mod.rs", 'pub(crate) mod key {\n    cfg_select! {', '''pub(crate) mod key {
    cfg_select! {
        target_os = "hyper" => {
            mod racy;
            mod hyper;
            pub(super) use racy::LazyKey;
            pub(super) use hyper::{Key, get, set};
            use hyper::{create, destroy};
        }''')

# The OS-key backend wraps even explicit const initializers in a function;
# Clippy cannot infer the original const block from that expansion.
replace("std/src/sys/thread_local/os.rs", "        #[inline]\n        fn __rust_std_internal_init_fn()",
        '        #[inline]\n        #[cfg_attr(target_os = "hyper", allow(clippy::missing_const_for_thread_local))]\n        fn __rust_std_internal_init_fn()')

replace("std/src/sys/time/mod.rs", "cfg_select! {", '''cfg_select! {
    target_os = "hyper" => {
        #[path = "hyper.rs"]
        mod imp;
    }''')

# Like upstream wasm's fallback, HashMap keys use allocation addresses. This
# does not implement a cryptographic entropy source: fill_bytes still panics.
replace("std/src/sys/random/mod.rs", '        target_os = "vexos",', '        target_os = "hyper",\n        target_os = "vexos",')
replace("std/src/sys/random/mod.rs", '\n    target_os = "vexos",', '\n    target_os = "hyper",\n    target_os = "vexos",')

replace("std/src/sys/env_consts.rs", '// The fallback when none of the other gates match.', '''#[cfg(target_os = "hyper")]
pub mod os {
    pub const FAMILY: &str = "";
    pub const OS: &str = "hyper";
    pub const DLL_PREFIX: &str = "lib";
    pub const DLL_SUFFIX: &str = ".so";
    pub const DLL_EXTENSION: &str = "so";
    pub const EXE_SUFFIX: &str = "";
    pub const EXE_EXTENSION: &str = "";
}

// The fallback when none of the other gates match.''')

replace("std/src/os/mod.rs", "pub mod raw;", 'pub mod raw;\n#[cfg(target_os = "hyper")]\npub mod hyper;')
