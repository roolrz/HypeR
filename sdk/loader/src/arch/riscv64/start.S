/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

.section .text._start, "ax", @progbits
.p2align 2
.global _start
.type _start, @function
_start:
    mv s0, sp
    mv a0, sp
    call __hyper_rtld_start
    mv sp, s0
    li s0, 0
    li ra, 0
    jr a0
.size _start, . - _start
.section .note.GNU-stack, "", @progbits
