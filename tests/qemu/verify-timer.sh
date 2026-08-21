#!/bin/sh
# Exercises recurring timer delivery on the supported four-core QEMU setup.
set -eu

if [ "$#" -ne 6 ]; then
    echo "usage: verify-qemu-timer.sh QEMU IMAGE INITRD CPU MEMORY BOOTARGS" >&2
    exit 2
fi

qemu=$1
image=$2
initrd=$3
cpu=$4
memory=$5
bootargs=$6
log=$(mktemp -t hyper-qemu-timer.XXXXXX)
pid=

cleanup() {
    if [ -n "$pid" ]; then
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
    fi
    rm -f "$log"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

"$qemu" \
    -machine virt,virtualization=on,gic-version=3,dtb-randomness=on \
    -cpu "$cpu" \
    -smp 4 \
    -m "$memory" \
    -nodefaults \
    -display none \
    -serial stdio \
    -monitor none \
    -no-reboot \
    -append "$bootargs" \
    -initrd "$initrd" \
    -kernel "$image" >"$log" 2>&1 &
pid=$!

attempt=0
while [ "$attempt" -lt 300 ]; do
    if grep -q 'HypeR: SMP online: 4/4 discovered CPUs' "$log" &&
        grep -q 'HypeR: periodic timer IRQs active on 4 CPUs' "$log" &&
        grep -q '<6>\[[0-9][0-9]*\] HypeR: early console initialized' "$log" &&
        grep -q 'HypeR: scheduler active on bootstrap thread 0' "$log" &&
        grep -q 'HypeR test: scheduler ready/wait queues and sleeping sync passed' "$log" &&
        grep -q 'HypeR test: guarded thread, IRQ, and emergency stacks passed' "$log" &&
        grep -q 'HypeR test: fatal-path readiness contract passed' "$log" &&
        grep -q 'HypeR: architectural timer: host INTID 26, guest INTID 27 (host VIRQ [0-9][0-9]*), [1-9][0-9]* Hz tick from a [1-9][0-9]* Hz counter' "$log" &&
        grep -q 'HypeR: monotonic clocksource active at [1-9][0-9]* Hz' "$log" &&
        grep -q 'HypeR: virtual architected timer injection validated' "$log" &&
        grep -q 'HypeR: guest synchronous trap and vSysReg emulation validated' "$log" &&
        grep -q 'HypeR: kallsyms resolved hyper_kallsyms_lookup at 0x[0-9a-f][0-9a-f]*' "$log" &&
        grep -q 'HypeR: kernel log ring: 65536 bytes, 0 records dropped' "$log" &&
        grep -q 'HypeR: CPU power interface version .*: on=true, off=true, suspend=true, reset=true' "$log" &&
        grep -q 'HypeR: platform bus: .* bound, .* unmatched, .* deferred, .* failed' "$log" &&
        grep -q "HypeR: loaded VM 'alpine' from boot ramdisk: 128 MiB RAM, 1 vCPU(s)" "$log" &&
        grep -q 'HypeR: kernel initialization complete; starting Linux guest' "$log" &&
        grep -q 'HypeR: vCPU 0 running as scheduler thread [1-9][0-9]* on guarded stack 0x[0-9a-f][0-9a-f]*-0x[0-9a-f][0-9a-f]*' "$log" &&
        grep -q 'arch_timer: cp15 timer running at .* (virt).' "$log" &&
        grep -q 'HypeR guest: Linux userspace is running' "$log"; then
        echo "verified EL2 host ticks and the Linux virtual timer using model $cpu"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        cat "$log" >&2
        echo "QEMU exited before recurring timer IRQs were observed" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 0.1
done

cat "$log" >&2
echo "timed out waiting for four-CPU recurring timer IRQs on QEMU CPU $cpu" >&2
exit 1
