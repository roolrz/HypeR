<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native Rust standard library

The assembled SDK supports ordinary Rust `fn main()` applications on
`aarch64-unknown-hyper` and `riscv64-unknown-hyper`. `hyper-cargo` builds the matching `std` automatically;
applications do not need a custom entry point, panic handler, or allocator.

```sh
target/sdk/aarch64/bin/hyper-cargo generate-lockfile --manifest-path path/to/Cargo.toml
target/sdk/aarch64/bin/hyper-cargo build --manifest-path path/to/Cargo.toml --release --locked
```

The output is in Cargo's `<architecture>-unknown-hyper/release` directory. Use
`HYPER_LINK_MODE=static` for static PIE. Existing freestanding applications
using `hyper_rt::entry!` select `HYPER_RUST_STD=0`, which retains the
`aarch64-unknown-none` or `riscv64gc-unknown-none-elf` target and `core,alloc` build. A std application that
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
| Files | Read/write/create/append/truncate, seek, metadata/times/permissions, rename/copy, hard links, symlink queries, directory traversal and recursive removal, advisory locks and sync |
| Paths and cwd | Canonicalization, current-directory observation and mutation through rooted Directory scopes |
| Subprocesses | Native ProcessBuilder, arguments/environment, cwd capability, wait/try_wait/kill, piped or inherited byte-channel stdio |
| Networking | Upstream unsupported implementation |
| Wall-clock time | RTC-anchored UTC when available; absent platform clocks retain unsupported behavior |
| Environment mutation | Upstream unsupported behavior |
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
The shared `libhyper` owns startup state, the current directory, input buffering,
the heap, and TLS primitives. Thus separately linked shims do not create separate process
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
starts with a zero thread pointer (`TPIDR_EL0` on AArch64, `tp` on RV64); the shared runtime attaches an allocated thread
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

## Filesystem semantics

The shared runtime retains authorized duplicates of startup directory, task
and stdio capabilities before application code can take its startup handles.
The rooted std filesystem namespace requires an explicit startup root grant;
a cwd grant alone does not implicitly become a process root. Such callers can
still use confined Native Directory APIs. A supplied root cursor is normalized
at its current node, and that same process root governs absolute paths, cwd
derivation and child delegation. Relative paths start at the delegated
cwd, or root when no cwd is supplied; absolute paths and absolute symlink targets
start at the delegated root. Parent traversal cannot escape that root.

The cwd owner lives in `libhyper`. `env::set_current_dir` replaces an owned
Directory scope; concurrent operations retain their own capability snapshots.
`env::current_dir` and `fs::canonicalize` resolve the current namespace spelling,
including directory renames, rather than returning a cached lexical path.

`File::try_clone` shares the adapter's offset and Native File owner. Append
chooses the end atomically for each short write. Open files survive unlink and
rename replacement. Rename, hard links, symbolic links, permissions, timestamps,
copy, and recursive directory removal are supported. `fs::copy` also applies the
source permission bits. `fs::metadata` follows symbolic links, while
`fs::symlink_metadata` and `DirEntry::metadata` inspect the final link itself.
`std::os::hyper::fs` provides symbolic-link creation and Native mode extensions.

Mode bits control admission of new read/write/execute capabilities; already
granted handles keep their authority. Metadata changes and advisory file locks
require separate `SET_ATTRIBUTES` and `LOCK_FILE` rights. Clones share one lock
owner, independent opens compete, and final active handle closure releases the
grant. Contended upgrades preserve the shared grant and return an error;
callers can unlock before requesting a blocking exclusive lock.

The current filesystem is volatile ramfs. `sync_all` and `sync_data` acknowledge
completed in-memory changes without promising persistence across restart.
Recursive deletion uses pinned directory capabilities and conditional removal,
so replacing an entry with a symlink cannot redirect traversal into its target.
Concurrent namespace mutation can still make the operation fail partway through.

Timestamps use signed UTC seconds and nanoseconds. Unavailable metadata times
return `Unsupported` instead of fabricated epoch or uptime values. A missing
platform wall clock makes the infallible `SystemTime::now` panic; explicit
metadata times remain usable without an RTC. Shared file mappings, Unix
credentials, and file descriptors are not provided by these interfaces.

## Subprocesses

Spawning requires TaskFactory, TaskGroup and ResourceDomain capabilities in
addition to the filesystem authority needed to open the executable. Missing
authority is an error; std does not acquire ambient capabilities.

`Command` supports PATH search, arguments, environment overrides, a delegated
child cwd and Native lifecycle observation. Its cwd is resolved through
Directory capabilities before relative executable lookup. Global environment
mutation remains unsupported. `output` and `wait_with_output` drain stdout and
stderr concurrently using a bounded WaitSet. Stdio inheritance uses explicitly
duplicatable ByteChannels; `Stdio::null` drains output on a runtime thread and
supplies EOF for input. File-backed stdio is unsupported because Native
ProcessBuilder currently accepts channels for these services.

`process::id` and child IDs expose observation-only kernel object identities;
Rust's u32 ID surface cannot represent a KOID above u32::MAX and rejects that
case rather than aliasing identities. Signals, fork, exec-in-place and Unix
process groups are not provided.

Native acceptance checks cover file content and namespace changes, metadata,
UTC progression, rename-stable cwd, recursive deletion around symlinks, and
file-lock cleanup across thread and process exit. They also exercise one-shot
WaitSet rearm and peer close, more than 64 persistent sources, and child output
larger than channel capacity on both streams. Both static and dynamic std
applications use the assembled SDK.

## Architecture selection

The installed SDK selects the target; see [toolchain architecture selection](../README.md#architecture-selection).
RV64 uses LP64D and a 16-byte aligned stack. Both architectures share this std
PAL and key-based TLS implementation; neither currently supports compiler ELF TLS.

### Native file interoperability

`std::os::hyper::io::FromRawHandle` lets trusted adapters transfer an exclusively
owned Native File handle into `std::fs::File`. Its unsafe contract requires a
live, uniquely owned File capability. Applications should use the safe
`hyper_os::fs::File::into_std()` adapter with the SDK's `std` feature instead.
The transfer preserves rights, starts the stream offset at zero and disables
append mode; clones share the std stream offset and close only on last drop.
`std::os::hyper::fs::FileExt::{read_at,read_exact_at}` perform positioned reads
without changing that offset. No whole-VM, capability-channel or WaitSet API
is moved into std by this interoperability boundary.
