<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native integration test application

This freestanding application exercises only public HypeR ABI and Lib
interfaces. The top-level SDK check consumes it as the stable application
input for assembled SDK and compiler integration checks.

The application is deliberately separate from `tests/unit`: it is not a unit
test for library implementation details. The integration pipeline currently
compiles it as an AArch64 PIE object with compiler builtins disabled, preserving
its library dependencies for the link stage. It can become a linked and
executable acceptance image without changing its source-level interface once
the Native startup and static PIE link contracts are available.
