// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(target_os = "hyper", target_arch = "riscv64"))]
mod runtime;

fn main() -> std::process::ExitCode {
    #[cfg(all(target_os = "hyper", target_arch = "riscv64"))]
    return runtime::main();
    #[cfg(not(all(target_os = "hyper", target_arch = "riscv64")))]
    {
        eprintln!("vm-smoke requires a RISC-V HypeR /init bootstrap");
        std::process::ExitCode::FAILURE
    }
}
