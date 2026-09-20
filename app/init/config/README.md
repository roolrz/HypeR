<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Init configuration

`services.json` is the production I/O VM service template. Board packaging reads
`boards/<board>.json` and generates the installed manifest, including storage
readiness and broker endpoints. Edit the board configuration for VM definitions;
its generated `vms.json` lives on the configuration volume at `/data`.

`native/services.json` and `native/vms.json` provide the standalone Native profile
without an I/O VM, used for early hardware bring-up.

Test manifests live in `../tests/config/`. They may autostart test guests and are
not the production board configuration.

Init supervises the critical `session` virtual console manager, not `/bin/sh`.
The manager establishes its console before spawning a shell, supplies a fresh
set of client channels, and supervises that noncritical client independently.
Shell exit or a fault starts another shell; rapid failures are rate-limited.
Console transport failure remains a service failure. Each manager instance owns
only its assigned console channels, allowing future serial devices to have
independent managers and foreground shells. The current bootstrap wires one
physical console. A manager may take an absolute shell executable as its optional
first argument (default `/bin/sh`); this does not yet add a multi-UART device-discovery API.
