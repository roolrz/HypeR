/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

.section .text._start, "ax", %progbits
.p2align 2
.global _start
.type _start, %function
_start:
    mov x19, sp
    mov x0, sp
    bl __hyper_rtld_start
    mov sp, x19
    mov x29, xzr
    mov x30, xzr
    br x0
.size _start, . - _start

.section .note.GNU-stack, "", %progbits
