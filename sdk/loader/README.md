<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR runtime linker

This component provides the AArch64 Native ELF interpreter installed as
`/lib/ld-hyper-aarch64.so`. The kernel maps only the main image and this
interpreter. Dependency policy, symbol lookup, relocation, constructors,
RELRO, and runtime loading remain userspace responsibilities.

Library lookup is capability-relative: the interpreter receives a Directory
handle and accepts exact relative object names, never ambient search paths.
Initial dependencies are global; `hyper_dlopen_at` additionally supports local
symbol scopes. Loading is eager and bounded, with AArch64 `RELA` and `RELR`
support and no writable-executable mapping transition.

`hyper_dlclose` currently releases a logical caller reference but deliberately
does not run destructors or reclaim mappings. This conservative lifetime rule
avoids invalidating code while other threads may still execute it; unload will
require an explicit process-wide quiescence protocol.
