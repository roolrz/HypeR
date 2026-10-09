#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Reject FP/SIMD instructions outside the kernel's three owned-state leaves."""

import re
import subprocess
import sys

ALLOWED = {'aarch64_fp_reset', 'aarch64_fp_restore', 'aarch64_fp_save_and_clear'}
SYMBOL = re.compile(r'^[0-9a-f]+ <([^>]+)>:$')
INSTRUCTION = re.compile(r'^\s*[0-9a-f]+:\s+(\S+)\s*(.*)$')
# General-register-only code must not name scalar, SIMD, SVE, or predicate
# registers, nor FP control/status registers. Decode the final linked image,
# including compiler builtins; inspecting only Rust source is insufficient.
FP_OPERAND = re.compile(r'\b(?:[bhsdqvz](?:[12]?[0-9]|3[01])|p(?:[0-9]|1[0-5])|fpcr|fpsr|fpmr|svcr)\b')


def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: verify-aarch64-fp.py LLVM_OBJDUMP ELF')
    text = subprocess.check_output(
        [sys.argv[1], '--disassemble', '--no-show-raw-insn', sys.argv[2]], text=True)
    symbol = None
    seen = set()
    bad = []
    for line in text.splitlines():
        match = SYMBOL.match(line)
        if match:
            symbol = match[1]
            continue
        # LLVM retains raw bytes for mapping-symbol data even with
        # --no-show-raw-insn; these are literal pools, not instructions.
        if '\t.word\t' in line or '\t.xword\t' in line or '\t.byte\t' in line:
            continue
        match = INSTRUCTION.match(line)
        if not match or not FP_OPERAND.search(match[2]):
            continue
        if symbol in ALLOWED:
            seen.add(symbol)
        else:
            bad.append(f'{symbol}: {line.strip()}')
    if bad:
        raise SystemExit('unowned kernel FP/SIMD instructions:\n' + '\n'.join(bad[:30]))
    if seen != ALLOWED:
        raise SystemExit(f'missing audited FP-state leaves: {sorted(ALLOWED - seen)}')
    print('AArch64 kernel FP/SIMD ownership audit passed')


if __name__ == '__main__':
    main()
