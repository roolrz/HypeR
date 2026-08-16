# RISC-V 64-bit port

## Supported profile

The initial port deliberately targets one useful virtualization profile rather
than every historical RISC-V board:

- RV64GC host execution with the H extension
- Sv39 HS address translation and Sv39x4 guest-stage translation
- SSTC supervisor timer compare for guest virtual timers
- SBI base, TIME, IPI, RFENCE, HSM, and SRST firmware services
- QEMU `virt`, OpenSBI, PLIC, ACLINT timers, and NS16550 early console
- four host harts and one Linux guest vCPU

HypeR validates the firmware DTB for PLIC, timebase, and SSTC data before using
those facilities. Missing mandatory virtualization or timer facilities are a
boot-time platform error; silently selecting a weaker execution model would
make guest behavior depend on accidental QEMU defaults.

The Rust target remains `riscv64imac-unknown-none-elf`. Ordinary Rust kernel
code therefore has a conservative soft-float baseline. Clang compiles the
small architecture assembly set with RV64GC+H but `-mabi=lp64`, keeping object
ABIs compatible while isolating H, F, and D instructions. Guest floating-point
state is initialized deterministically and has an explicit save/restore
context.

## Architecture boundaries

Architecture code owns entry state, CSRs, traps, SBI instruction bridges,
Sv39 page tables, Sv39x4 guest translation, TLB fences, timer compare state,
and vCPU register layout. The shared HAL exposes semantic memory, interrupt,
timer, CPU-power, barrier, and cache contracts only. PLIC, NS16550, and SBI
policy live in reusable driver or platform layers; QEMU-specific discovery
selects them without exposing CSR details to the kernel.

The per-VM interrupt object has separate AArch64 and RISC-V implementations.
RISC-V does not instantiate the GIC/vGIC model merely to satisfy a shared type.
HVIP supplies the initial virtual local-interrupt mechanism; a future AIA
backend can add virtual IMSIC state behind that RISC-V implementation.

## Linux guest ABI

Linux enters VS mode at `0x80200000` with hart ID zero in `a0` and the guest
DTB address in `a1`. Guest RAM begins at `0x80000000`; the DTB is placed at
`0x80010000`. The virtual firmware implements the SBI operations required by
the current uniprocessor boot, and Linux receives RV64GC, Sv39, and SBI nodes
in a Linux-format DTB. The smoke-test command line uses `keep_bootcon` because
the intentionally minimal virtual board has no runtime UART yet; this keeps
Linux's SBI boot console observable through `/init` without advertising a
nonexistent `hvc0` device.

The CI smoke test downloads checksum-pinned Alpine artifacts, boots a four-hart
HypeR host, enters the Linux guest, and requires the kernel to execute `/init`.
The downloaded GPL kernel and distribution files remain ignored external test
artifacts and are not part of the Apache-2.0 source tree.

## Current limitations

- QEMU `virt` is the only supported RISC-V board.
- PLIC supervisor context numbering follows the QEMU/legacy SiFive ordering;
  parsing `interrupts-extended` is required before supporting arbitrary PLIC
  topologies.
- SSTC is mandatory; the software-injected fallback is not a supported profile.
- The Linux guest is uniprocessor and has no virtual PLIC, AIA, UART, virtio,
  block, or network device.
- Guest WFI currently traps to HS and resumes cooperatively. A scheduler-aware
  blocked-vCPU path is required before guest timeslicing.
- Cache maintenance assumes QEMU's coherent platform. Real non-coherent RISC-V
  platforms need a discoverable cache-block-management or platform backend.
