#!/bin/sh
# Boots the four-hart RISC-V host and its Linux guest through /init.
set -eu

if [ "$#" -ne 6 ]; then
    echo "usage: verify-riscv64.sh QEMU IMAGE INITRD CPU MEMORY BOOTARGS" >&2
    exit 2
fi

qemu=$1
image=$2
initrd=$3
cpu=$4
memory=$5
bootargs=$6
timeout_seconds=${QEMU_BOOT_TIMEOUT_SECONDS:-180}
temp=$(mktemp -d -t hyper-qemu-riscv64.XXXXXX)
log=$temp/output.log
pid=

case "$timeout_seconds" in
    ''|*[!0-9]*|0)
        echo "QEMU_BOOT_TIMEOUT_SECONDS must be a positive integer" >&2
        exit 2
        ;;
esac
attempt_limit=$((timeout_seconds * 10))

magic=$(dd if="$image" bs=1 skip=56 count=4 2>/dev/null | od -An -tx1 | tr -d ' \n')
if [ "$magic" != "52534305" ]; then
    echo "HypeR image does not contain a RISC-V Linux Image header" >&2
    exit 1
fi

cleanup() {
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
    fi
    if [ -n "$pid" ]; then
        wait "$pid" 2>/dev/null || true
    fi
    rm -rf "$temp"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

"$qemu" \
    -machine virt \
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
while [ "$attempt" -lt "$attempt_limit" ]; do
    if grep -q 'HypeR: early console initialized' "$log" &&
        grep -q 'HypeR: interrupt controller initialized with [1-9][0-9]* interrupt IDs' "$log" &&
        grep -q 'HypeR: CPU power interface version .*: on=true, off=true, suspend=true, reset=true' "$log" &&
        grep -q 'HypeR: architectural timer: host INTID 0, guest INTID 5' "$log" &&
        grep -q 'HypeR: RISC-V guest SBI and virtual timer backend initialized' "$log" &&
        grep -q 'HypeR: SMP online: 4/4 discovered CPUs' "$log" &&
        grep -q 'HypeR: transition identity mappings retired' "$log" &&
        grep -q "HypeR: loaded VM 'alpine' from boot ramdisk: 128 MiB RAM, 1 vCPU(s)" "$log" &&
        grep -q 'Booting Linux on hartid 0' "$log" &&
        grep -q 'riscv: ELF capabilities acdfim' "$log" &&
        grep -q 'Run /init as init process' "$log"; then
        echo "verified four-hart RISC-V host and Linux guest /init on QEMU CPU $cpu"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        cat "$log" >&2
        echo "QEMU exited before the RISC-V Linux guest reached /init" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 0.1
done

cat "$log" >&2
echo "timed out after ${timeout_seconds}s waiting for the RISC-V Linux guest to reach /init" >&2
exit 1
