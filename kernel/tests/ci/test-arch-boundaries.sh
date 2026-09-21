#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

set -eu
scripts=$(CDPATH='' cd -- "$(dirname "$0")" && pwd)
exec python3 -B "$scripts/test-hal-boundary.py" GraphTests
