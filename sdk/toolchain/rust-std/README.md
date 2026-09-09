<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native Rust standard library

The assembled SDK supports ordinary Rust `fn main()` applications on
`aarch64-unknown-hyper`. `hyper-cargo` builds the matching `std` automatically;
applications do not need a custom entry point, panic handler, or allocator.

```sh
target/sdk/aarch64/bin/hyper-cargo generate-lockfile --manifest-path path/to/Cargo.toml
target/sdk/aarch64/bin/hyper-cargo build --manifest-path path/to/Cargo.toml --release --locked
```

The output is in Cargo's `aarch64-unknown-hyper/release` directory. Use
`HYPER_LINK_MODE=static` for static PIE. Existing freestanding applications
using `hyper_rt::entry!` select `HYPER_RUST_STD=0`, which retains the
`aarch64-unknown-none` target and `core,alloc` build. A std application that
depends on `hyper-rt` must enable its `std` feature to avoid defining a second
panic handler; it should still use ordinary `main`, not `hyper_rt::entry!`.

The [acceptance application](../tests/std-smoke/src/main.rs) uses unmodified
clap with default features, derive, and environment lookup. Its procedural
macros run on the host compiler; its application and ordinary dependencies
are compiled for HypeR. Run `make test-native` to exercise it through the
Native shell, including arguments, help, invalid arguments, panic, standard
input, output, and static/dynamic linking.

## Supported surface

| Facility | Current behavior |
| --- | --- |
| Collections, formatting, `System` allocator | Shared Native process heap |
| Arguments and environment reads | Immutable startup bytes; iterators own snapshots |
| stdin/stdout/stderr | Native service byte channels; Console fallback for bootstrap programs |
| `Instant`, sleep, timed waits | Native monotonic clock; yielding wait fallback |
| Mutex, RwLock, Condvar, Once, parking | Upstream atomic/futex algorithms with Native wait/wake bridge |
| `thread_local!` | Key-based TLS in a per-thread control block; bounded destructor passes |
| `thread::Builder::spawn` | `io::ErrorKind::Unsupported`; failed creation releases captured values |
| Files, networking, subprocess creation | Upstream unsupported implementations |
| Wall-clock time, environment mutation | Upstream unsupported behavior, including panic where the public API cannot return an error |
| Cryptographic randomness | Unsupported; no entropy source is claimed |
| HashMap seeds | Upstream unsupported-target address-based fallback; no strong collision-attack resistance claim |
| Terminal detection | Upstream fallback reports false; terminal sizing is unavailable |
| Panic | Abort; diagnostics use stderr, no unwinding or symbolized backtrace |

This is a partial std platform port. Crates that directly depend on Unix,
libc, native TLS, or another OS-specific backend still require their own
HypeR support. Unused unsupported APIs remain available for compilation;
calling them follows each upstream API's error/panic contract.

## Ownership and build boundary

`prepare-rust-std.py` copies the installed rust-src into the SDK staging area
and applies checked platform-selection edits plus the files in `overlay/`.
It never mutates rustup's installation. Both preparation and consumption
require Rust 1.97.1. Upgrading Rust requires reviewing these anchors and
rerunning the Native acceptance suite, not merely changing the version pin.

The platform adapter contains Rust-private type conversions. Its C calls go
through `libhyper-std.a`, which contains stateless std-specific translation.
The shared `libhyper` owns startup state, input buffering, the heap, and TLS
primitives. Thus separately linked shims do not create separate process
registries. The bridge headers and Rust adapter are versioned with the SDK;
they are not new kernel syscalls or a promise of permanent binary stability.

The driver uses Cargo's private `__CARGO_TESTS_ONLY_SRC_ROOT` override and
unstable JSON-target/build-std support under scoped `RUSTC_BOOTSTRAP=1`.
These are explicit pinned-toolchain dependencies. Third-party dependencies
remain lockfile-controlled. The SDK checks fetch their locked inputs before
performing offline builds. The standard library source remains under its
upstream licensing terms.

## Next thread patch

No component assumes that a process has only one thread. A Native thread
starts with `TPIDR_EL0 == 0`; the shared runtime attaches an allocated thread
control block. The kernel already preserves this register across user
context switches. Rust uses the OS-key TLS implementation, not compiler
ELF TLS. Native ELF TLS relocations and PT_TLS remain outside this patch.

`hyper_runtime_tls_*` uses process-wide, non-reused keys and per-thread
values. Key metadata lives until process exit so that deletion cannot alias
stale values in another thread; values and thread storage are reclaimed at
detach. Destructors run without a runtime lock, clear their slot before the
callback, and may reinitialize slots for a bounded number of passes. Normal
return from main detaches the initial thread; process abort/exit need not
run destructors. Unloading code that owns live TLS destructors is not supported.

The next backend must implement the following contracts behind the existing
shim signatures:

1. Spawn failure keeps the entry argument with the caller. Success transfers
   it to exactly one child, with a suitably aligned owned stack and a join
   token. The child attaches TLS before invoking Rust's entry callback.
2. Returning from the callback runs TLS detach/destructors, then exits the
   thread. Stack reclamation happens only once the thread has stopped using
   it. Join and detached-thread cleanup must coordinate this ownership.
3. Address wait atomically checks the expected u32 and registers the waiter.
   It handles cancellation, spurious wakeups, and absolute monotonic deadlines.
   Wake follows the caller's release operation. Current waits poll/yield and
   observe state changes directly; replacing this fallback must not introduce
   a lost-wakeup gap. It is not an efficient blocking implementation yet.

Host tests run simultaneous TLS clients and destructors using pthreads, and
check wait state changes and deadlines. Native tests cover the initial thread
and migration through scheduler waits; Native thread creation is explicitly
not claimed until the syscall patch lands.
