#!/bin/sh
# Exercises the complete supported four-core QEMU runtime configuration.
set -eu

if [ "$#" -ne 6 ]; then
    echo "usage: verify-qemu-smp.sh QEMU IMAGE INITRD CPU MEMORY BOOTARGS" >&2
    exit 2
fi

qemu=$1
image=$2
initrd=$3
cpu=$4
memory=$5
bootargs=$6
case "$cpu" in
    cortex-a72) host_mode='nVHE' ;;
    max) host_mode='VHE' ;;
    *) host_mode='\(nVHE\|VHE\)' ;;
esac
temp=$(mktemp -d -t hyper-qemu-smp.XXXXXX)
log=$temp/output.log
input=$temp/input
pid=
input_sent=false

mkfifo "$input"
exec 3<>"$input"

cleanup() {
    if [ -n "$pid" ]; then
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
    fi
    rm -rf "$temp"
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
    -kernel "$image" <"$input" >"$log" 2>&1 &
pid=$!

attempt=0
while [ "$attempt" -lt 300 ]; do
    if [ "$input_sent" = false ] && grep -q 'HypeR guest: Linux userspace is running' "$log"; then
        # BusyBox ash asks the terminal for the cursor position before reading
        # its first command. Answer that query, then exercise guest RX.
        printf '\033[1;1Recho RX_OK\n' >&3
        input_sent=true
    fi
    if grep -q '<6>\[[0-9][0-9]*\] HypeR: early console initialized' "$log" &&
        grep -q 'HypeR: scheduler active on bootstrap thread 0' "$log" &&
        grep -q 'HypeR test: scheduler ready/wait queues and sleeping sync passed' "$log" &&
        grep -q 'HypeR test: guarded thread, IRQ, and emergency stacks passed' "$log" &&
        grep -q 'HypeR: kallsyms resolved hyper_kallsyms_lookup at 0x[0-9a-f][0-9a-f]*' "$log" &&
        grep -q 'HypeR: kernel log ring: 65536 bytes, 0 records dropped' "$log" &&
        grep -q 'HypeR: CPU power interface version .*: on=true, off=true, suspend=true, reset=true' "$log" &&
        grep -q 'HypeR: vGICv3 active with [1-9][0-9]* LRs, [5-8] priority bits, [5-7] preemption bits, \(16\|24\) INTID bits, maintenance VIRQ [0-9][0-9]*' "$log" &&
        grep -q 'HypeR: architectural timer: host INTID 26, guest INTID 27 (host VIRQ [0-9][0-9]*), [1-9][0-9]* Hz tick from a [1-9][0-9]* Hz counter' "$log" &&
        grep -q 'HypeR: monotonic clocksource active at [1-9][0-9]* Hz' "$log" &&
        grep -q 'HypeR: virtual architected timer injection validated' "$log" &&
        grep -q 'HypeR: guest synchronous trap and vSysReg emulation validated' "$log" &&
        grep -q 'HypeR: platform bus: .* bound, .* unmatched, .* deferred, .* failed' "$log" &&
        grep -q 'HypeR: CPU 1 online, hardware ID 0x1; entering idle' "$log" &&
        grep -q 'HypeR: CPU 2 online, hardware ID 0x2; entering idle' "$log" &&
        grep -q 'HypeR: CPU 3 online, hardware ID 0x3; entering idle' "$log" &&
        grep -q 'HypeR: SMP online: 4/4 discovered CPUs' "$log" &&
        grep -q 'HypeR: randomized kernel base 0x[0-9a-f][0-9a-f]*, KASLR offset 0x[0-9a-f][0-9a-f]*' "$log" &&
        grep -q 'HypeR: transition identity mappings retired' "$log" &&
        grep -q "HypeR: AArch64 host execution mode: $host_mode" "$log" &&
        grep -q "HypeR: loaded VM 'alpine' from boot ramdisk: 128 MiB RAM, 1 vCPU(s)" "$log" &&
        grep -q 'HypeR: kernel initialization complete; starting Linux guest' "$log" &&
        grep -q 'HypeR: vCPU 0 running as scheduler thread [1-9][0-9]* on guarded stack 0x[0-9a-f][0-9a-f]*-0x[0-9a-f][0-9a-f]*' "$log" &&
        grep -q 'HypeR: periodic timer IRQs active on 4 CPUs' "$log" &&
        grep -q 'Booting Linux on physical CPU' "$log" &&
        grep -q 'arch_timer: cp15 timer running at .* (virt).' "$log" &&
        grep -q 'Run /init as init process' "$log" &&
        grep -q 'HypeR guest: /init reached' "$log" &&
        grep -q 'HypeR guest: Linux userspace is running' "$log" &&
        grep -q '^RX_OK' "$log"; then
        kaslr_base=$(sed -n 's/.*randomized kernel base \(0x[0-9a-f][0-9a-f]*\),.*/\1/p' "$log" | tail -n 1)
        kaslr_offset=$(sed -n 's/.*KASLR offset \(0x[0-9a-f][0-9a-f]*\).*/\1/p' "$log" | tail -n 1)
        kaslr_base_value=$((kaslr_base))
        kaslr_offset_value=$((kaslr_offset))
        if [ $((kaslr_offset_value % 0x200000)) -ne 0 ] ||
            [ "$kaslr_offset_value" -ge $((512 * 1024 * 1024 * 1024)) ] ||
            [ "$kaslr_base_value" -ne $((0xff0000000000 + kaslr_offset_value)) ]; then
            cat "$log" >&2
            echo "invalid AArch64 KASLR offset: $kaslr_offset" >&2
            exit 1
        fi
        echo "verified host SMP/KASLR and Linux guest init on QEMU CPU $cpu"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        cat "$log" >&2
        echo "QEMU exited before SMP startup completed" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 0.1
done

cat "$log" >&2
echo "timed out waiting for the Linux guest to reach init" >&2
exit 1
