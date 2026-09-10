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
| `hyper-rt` | Rust application entry, process-heap allocator, panic termination, and exit status |
| `hyper-service` | Shared typed startup contracts for Native system services |

Unsafe machine interactions are confined to `hyper-sys`. Application code
should normally depend only on `hyper-os` and `hyper-rt`. `hyper-os` is an
application-independent semantic layer and keeps Rust standard-library
platform adapters separate from its stable-facing capability interfaces.

The safe layer includes bounded byte streams, transactional capability
rendezvous, startup-capability parsing, directory access, physical and emergency
Console access, object waits, and staged Process construction. Rust owners keep
MOVE operations recoverable until the kernel commits them, while typed receive
slots validate object kind and exact rights before publishing a capability to
application code. `hyper-service` standardizes symbolic contract names and
typed startup purposes shared by providers and consumers; each service still
owns its message payload semantics, and the SDK imposes no generic IPC wire
envelope.

The initial runtime reuses the C startup parser and selected architecture syscall veneer from
`sdk/lib`. This preserves one machine entry contract while the Rust API is
established. Loader/CRT startup reserves a private heap VMAR before application entry, and
`hyper-rt` installs the `libhyper` process heap as Rust's global allocator.
Applications can use `alloc` without implementing an allocator:

```rust
extern crate alloc;

use alloc::{format, string::String, vec::Vec};

let name = String::from("shell");
let message = format!("hello from {name}");
let bytes: Vec<u8> = [b"hyper> ".as_slice(), message.as_bytes()].concat();
```

`hyper_rt::alloc` also re-exports the crate. This provides `String`, `Vec`,
`Box`, collections, `.concat()`, and `format!`; the freestanding mode does not provide `std` or
POSIX APIs. The allocator supports over-aligned Rust layouts, reclaims empty
mapped regions, and preserves the original buffer when reallocation fails.
Use fallible collection APIs such as `try_reserve` where OOM is recoverable;
infallible allocation failure follows Rust's allocation-error path and the
runtime's aborting panic policy. Memory use remains charged to the process's
resource domain. See [the C heap contract](../lib/README.md#process-heap).

`hyper-cargo` in freestanding mode rebuilds `core` and `alloc` as PIC for Native PIE linking using
the pinned compiler's `rust-src`. The driver locally enables Cargo's unstable
`build-std` through `RUSTC_BOOTSTRAP=1`; this is a toolchain dependency, not a
stable Rust target support claim. SDK assembly fetches the compiler-library
lockfile's dependencies before offline application builds.

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

## Standard Rust applications

The SDK now also provides a partial platform port of Rust `std`, including
ordinary `main()`, standard streams, startup arguments, and clap support.
`hyper-cargo` selects this mode by default. Existing `hyper_rt::entry!`
applications select `HYPER_RUST_STD=0`; std applications using this crate
enable its `std` feature and use ordinary `main()`. See the
[Native std guide](../toolchain/rust-std/README.md) for the support matrix,
build contract, and Native thread and atomic-wait backend.

Native std programs can call `hyper_rt::process::startup()` once to claim their
non-stream startup capabilities. Standard streams and the bootstrap Console
remain runtime-owned for std cleanup and TLS destructors. Prefer `std::io` for
ordinary I/O; `hyper_rt::process::{stdin,stdout,stderr,console}` expose borrowed
owners when a service needs Native waits, routing, or capability duplication.

Native `thread` bindings expose dormant creation, start, stop, typed termination
waits, and process-private `AtomicU32` wait/wake. Raw thread construction and
asynchronous stop require explicit unsafe lifetime contracts. Applications
should prefer `std::thread` and `std::sync` for language-level ownership.
Capability IPC and object waits remain Native APIs: std does not represent
VM control handles or transferable capability channels.


`fs::Directory` provides capability-relative creation/removal; `fs::File`
provides explicit-offset reads/writes, atomic append and resize. WRITE must be
present on the originating Directory before writable File authority can be
opened. Open owners survive unlink; enumeration returns owned metadata with
opaque, mutation-safe cursors. Prefer `std::fs` when path and stream operations
are sufficient; keep Native handles for explicit delegation and executable
provenance.

`wait::WaitSet` owns up to 1024 persistent one-shot subscriptions. `add` returns
an opaque non-reused registration ID; `wait` returns that ID, observed signals
and sequence. Consume before `rearm`, and use `remove` to cancel. Registration
management requires BIND_WAIT, consumption requires WAIT, and sources require
WAIT. Closing a set removes its subscriptions; source handle closure does not
cancel the retained object observation. WaitSet and CapabilityChannel sources
are currently rejected. ByteChannel endpoints can be explicitly duplicated;
peer close occurs only after the last active endpoint owner closes.
