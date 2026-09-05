<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Native ABI source

HypeR ABI is the compiler-checked source of truth for the HypeR Native syscall
boundary. It provides dependency-free Rust definitions for `no_std` consumers,
a C/C++ header, syscall metadata, wire-layout assertions, and an auditable
reference generated from one schema.

The Native ABI is intentionally independent of kernel implementation details
and language runtime policy. Linux, FreeBSD, POSIX, and other compatibility
interfaces are separate personalities and are not defined by this component.

HypeR is pre-release. The ABI revision remains zero until the project explicitly
publishes its first supported ABI; schema changes before then do not imply
stability.

## Repository layout

| Path | Responsibility |
| --- | --- |
| `schema/native.rs` | Machine-readable Native ABI schema |
| `src/generated.rs` | Generated dependency-free Rust values and layouts |
| `include/hyper/native.h` | Generated C and C++ interface |
| `docs/native.md` | Generated syscall and object reference |
| `generator/` | Schema validation and deterministic rendering |
| `tests/` | Rust, C, and C++ layout conformance |

## Use from Rust

The `hyper-abi` crate is `#![no_std]` and has no dependencies. The kernel uses
the in-tree package directly:

```toml
[dependencies]
hyper-abi = { path = "sdk/abi" }
```

## Generate and verify

```sh
cd sdk/abi
cargo run --features generator --bin hyper-abi -- write
cargo run --features generator --bin hyper-abi -- check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Generated files are reviewed and committed. CI rejects drift between the schema
and generated Rust, C, and reference outputs.

## Ownership boundary

- The kernel implements and validates the machine contract.
- `sdk/lib/` provides the C runtime and Native application interfaces.
- `sdk/toolchain/` assembles the compiler, linker, headers, and runtime into a
  consumable SDK.

## License

Licensed under the Apache License, Version 2.0. See
[the project license](../../LICENSE).
