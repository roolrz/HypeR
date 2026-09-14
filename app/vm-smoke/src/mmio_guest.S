// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
.section .rodata.hyper_mmio_guest,"a",@progbits
.balign 16
.global hyper_mmio_guest_start
.global hyper_mmio_guest_end
hyper_mmio_guest_start:
    mov x20, #0x0a000000
    mov w21, #32
1:
    mov w1, #0x1234
    str w1, [x20]
    ldrsb x2, [x20, #4]
    str x2, [x20, #8]
    ldr w3, [x20, #16]
    cmp w3, w1
    b.ne 3f
    subs w21, w21, #1
    b.ne 1b
    mov w1, #0x55
    str w1, [x20, #32]
2:
    wfi
    b 2b
3:
    mov w1, #0xff
    str w1, [x20, #32]
    b 2b
hyper_mmio_guest_end:
