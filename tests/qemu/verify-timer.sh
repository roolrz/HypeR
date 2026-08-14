#!/bin/sh
# Exercises recurring timer delivery on the supported four-core QEMU setup.
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: verify-qemu-timer.sh QEMU IMAGE CPU MEMORY" >&2
    exit 2
fi

qemu=$1
image=$2
cpu=$3
memory=$4
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
    -machine virt,virtualization=on,gic-version=3 \
    -cpu "$cpu" \
    -smp 4 \
    -m "$memory" \
    -nographic \
    -no-reboot \
    -kernel "$image" >"$log" 2>&1 &
pid=$!

attempt=0
while [ "$attempt" -lt 100 ]; do
    if grep -q 'HypeR: timer tick 3' "$log" &&
        grep -q 'HypeR: SMP online: 4/4 discovered CPUs' "$log" &&
        grep -q 'HypeR: CPU 1 timer tick 1' "$log" &&
        grep -q 'HypeR: CPU 2 timer tick 1' "$log" &&
        grep -q 'HypeR: CPU 3 timer tick 1' "$log" &&
        grep -q '<6>\[[0-9][0-9]*\] HypeR: early console initialized' "$log" &&
        grep -q 'HypeR: scheduler active on bootstrap thread 0' "$log" &&
        grep -q 'HypeR: kernel log ring: 65536 bytes, 0 records dropped' "$log" &&
        grep -q 'HypeR: CPU power interface version .*: on=true, off=true, suspend=true, reset=true' "$log" &&
        grep -q 'HypeR: platform bus: .* bound, .* unmatched, .* deferred, .* failed' "$log" &&
        grep -q 'HypeR: kernel initialization complete; bootstrap thread becoming idle' "$log"; then
        echo "verified recurring EL2 timer IRQs on four QEMU CPUs using model $cpu"
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
