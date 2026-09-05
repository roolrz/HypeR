// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::env;
use std::path::Path;
use std::process::Command;

#[test]
fn generated_c_header_satisfies_c_and_cpp_layout_assertions() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = manifest.join("include/hyper/native.h");
    let compiler = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let c_status = Command::new(&compiler)
        .args(["-std=c11", "-x", "c", "-fsyntax-only"])
        .arg(&header)
        .status();
    assert!(
        matches!(c_status, Ok(status) if status.success()),
        "generated C ABI header did not compile: {c_status:?}"
    );

    let cpp_status = Command::new(compiler)
        .args(["-std=c++17", "-x", "c++", "-fsyntax-only"])
        .arg(header)
        .status();
    assert!(
        matches!(cpp_status, Ok(status) if status.success()),
        "generated C++ ABI header did not compile: {cpp_status:?}"
    );
}
