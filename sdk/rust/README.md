<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Rust SDK

This component provides freestanding Rust bindings for applications compiled
directly against the HypeR Native ABI. The crates use `#![no_std]`, make no
Linux or POSIX assumptions, and are installed as source so consumers compile
them with the Rust toolchain selected for the application.

## Crates

| Crate | Responsibility |
| --- | --- |
| `hyper-sys` | Raw syscall, C-runtime, pointer, and handle bindings |
| `hyper-os` | Safe capability-oriented operating-system interfaces |
| `hyper-rt` | Rust application entry, panic termination, and exit status |

Unsafe machine interactions are confined to `hyper-sys`. Application code
should normally depend only on `hyper-os` and `hyper-rt`. `hyper-os` is an
application-independent semantic layer and is intended to support a future
HypeR port of the Rust standard library without adopting the standard
library's unstable internal platform interfaces as its own API.

The initial runtime reuses the C startup parser and AArch64 syscall veneer from
`sdk/lib`. This preserves one machine entry contract while the Rust API is
established. Heap allocation is not yet provided; applications currently use
`core` without `alloc` or `std`.

## Build boundary

The source workspace supports formatting, linting, and host-side tests:

```sh
cargo fmt --manifest-path sdk/rust/Cargo.toml --all -- --check
cargo clippy --manifest-path sdk/rust/Cargo.toml \
  --workspace --target aarch64-unknown-none --lib -- -D warnings
cargo test --manifest-path sdk/rust/Cargo.toml -p hyper-os -p hyper-sys
```

Applications do not use source-tree path dependencies. SDK assembly installs
these crates below `share/hyper/rust`, and `bin/hyper-cargo` supplies the
installed paths, target, linker, and PIE model. Repository builds select
Cargo's `--offline` mode; SDK consumers remain free to use separately reviewed
dependencies.

## License

Licensed under the Apache License, Version 2.0. See
[the project license](../../LICENSE).
