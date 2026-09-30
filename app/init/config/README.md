<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Init configuration

`services-with-vms.json` includes the console workers, session manager,
VM manager and I/O runtime. Board packaging reads
`boards/<board>.json` and generates the installed manifest, including storage
readiness and broker endpoints. Edit the board configuration for VM definitions;
its generated `vms.json` lives on the configuration volume at `/data`.

The board's `io-vm` object is also the sole source of I/O VM settings, including
its runtime, name, image path, configuration and device selector. Board and
standalone I/O builds install a board snapshot at `/etc/hyper/board.json`, which
io-runtime reads directly. Diskless bring-up derives an ordinary VM definition
from the selected Pi 5 board for manual startup without device authority.

`services-console-only.json` includes only the console workers and session
manager, used for early hardware bring-up and clock tests. These builds omit
`vms.json`; an empty VM configuration file is not needed.

The selected manifest is installed as `/etc/hyper/services.json` for init.

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
