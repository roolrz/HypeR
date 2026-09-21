#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

set -eu
scripts=$(CDPATH='' cd -- "$(dirname "$0")" && pwd)
root=${HYPER_ARCH_BOUNDARY_ROOT:-$(CDPATH='' cd -- "$scripts/../.." && pwd)}
exec python3 -B "$scripts/hal-boundary.py" graph --root "$root"
