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
| `Instant`, sleep, timed waits | Native monotonic clock and scheduler deadline parking |
| Mutex, RwLock, Condvar, Once, parking | Upstream atomic/futex algorithms with Native wait/wake bridge |
| `thread_local!` | Key-based TLS in a per-thread control block; bounded destructor passes |
| `thread::Builder::spawn` | Native thread creation, join, and detached-stack reclamation |
| Files | Read/write/create/append/truncate, seek, metadata, directory iteration/create/remove, copy |
| Subprocesses | Native ProcessBuilder, arguments/environment, cwd capability, wait/try_wait/kill, piped or inherited byte-channel stdio |
| Networking | Upstream unsupported implementation |
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

## Thread and synchronization runtime

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

The runtime creates a dormant Native Thread, publishes its token, then starts
it with an owned stack and entry argument. Failure keeps the argument with
the caller. The child attaches TLS before Rust entry and runs TLS destructors
before thread_exit. Join observes Native TERMINATED before freeing the stack.
A single process-lifetime cleanup worker performs the same wait for detached
threads; its own stack lives until Process retirement. Process exit may
abandon all remaining language destructors, as before.

The atomic wait bridge uses process-private u32 wait/wake syscalls. Kernel
mapping identities distinguish virtual-address reuse; pinned backing survives
unmap while an admitted call finishes. The predicate check and wait publication
share one sharded condition lock with wake. Existing scheduler generations
arbitrate notification, timeout and cancellation. Spurious wakeups are allowed;
upstream Rust locks retain their own acquire/release predicate loops. Shared
cross-process futexes and requeue are outside the current std contract.

Host tests exercise TLS destructors and upstream synchronization algorithms
against a host syscall substitute. Native tests exercise concurrent thread
creation, Mutex/Condvar progress, independent TLS, join and detached cleanup.
Physical AArch64 qualification must still stress weak ordering, migration,
concurrent mapping retirement and interrupt timing beyond QEMU coverage.

## Files and subprocesses

The shared runtime retains authorized duplicates of startup directory, task
and stdio capabilities before application code can take its startup handles.
Applications still need the corresponding authority: filesystem access uses
root/current-directory capabilities, and spawning additionally requires
TaskFactory, TaskGroup and ResourceDomain capabilities. A missing authority is
an error; std does not acquire an ambient root. Relative paths resolve beneath
the delegated cwd (or root when no cwd is supplied); absolute paths use the
delegated root. Parent traversal cannot escape either capability boundary.

`File::try_clone` shares the adapter's offset. Native append chooses the end of
the file atomically for each short write. File handles survive unlink. The
current filesystem is volatile ramfs; sync/durability, rename, links, canonical
paths, timestamps, permission mutation and shared file mappings return
Unsupported. `fs::copy` currently supports ordinary 0666 files only; it rejects
permission-preserving copies that need missing Native chmod instead of silently
changing permission bits. Global cwd/environment mutation remains unsupported.

`Command` supports PATH search, arguments, environment overrides, a delegated
child cwd and Native lifecycle observation. `output` and `wait_with_output`
drain stdout and stderr concurrently using a bounded WaitSet. Stdio inheritance
uses explicitly duplicatable ByteChannels; `Stdio::null` drains output on a
runtime thread and supplies EOF for input. File-backed stdio is unsupported
because Native ProcessBuilder currently accepts channels for these services.
`process::id` and child IDs expose observation-only kernel object identities;
Rust's u32 ID surface cannot represent a KOID above u32::MAX and rejects that
case rather than aliasing identities. Signals, fork, exec-in-place and Unix
process groups are not provided.

Native acceptance checks exercise sparse writes, truncation, concurrent append,
unlink/recreate lifetime, one-shot WaitSet rearm and peer close, more than 64
persistent sources, and child output larger than channel capacity on both
streams. Both static and dynamic std applications use the assembled SDK.

Native file metadata is available through `std::fs::metadata` and
`std::fs::symlink_metadata` (Native VFS currently has no symbolic links).
`std::os::hyper::fs::MetadataExt::mode()` exposes Native permission mode bits.
