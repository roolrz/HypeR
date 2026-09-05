#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

set -eu

if [ "$#" -ne 7 ]; then
    echo "usage: verify-native-init.sh QEMU IMAGE INITRAMFS CPU CPUS MEMORY BOOTARGS" >&2
    exit 2
fi

qemu=$1
image=$2
initramfs=$3
cpu=$4
cpus=$5
memory=$6
bootargs=$7
timeout_seconds=${QEMU_BOOT_TIMEOUT_SECONDS:-30}
temp=$(mktemp -d -t hyper-native-init.XXXXXX)
input=$temp/input
native_output=$temp/native-output
log=${QEMU_TEST_LOG:-$temp/output.log}
pid=

mkdir -p "$(dirname "$log")"
mkfifo "$input"
exec 3<>"$input"

cleanup() {
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        (
            sleep 2
            kill -KILL "$pid" 2>/dev/null || true
        ) &
        watchdog=$!
    else
        watchdog=
    fi
    if [ -n "$pid" ]; then
        wait "$pid" 2>/dev/null || true
    fi
    if [ -n "$watchdog" ]; then
        kill "$watchdog" 2>/dev/null || true
        wait "$watchdog" 2>/dev/null || true
    fi
    exec 3>&-
    rm -rf "$temp"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

"$qemu" \
    -machine virt,virtualization=on,gic-version=3,dtb-randomness=on \
    -cpu "$cpu" \
    -smp "$cpus" \
    -m "$memory" \
    -nodefaults \
    -display none \
    -serial stdio \
    -monitor none \
    -no-reboot \
    -append "$bootargs" \
    -initrd "$initramfs" \
    -kernel "$image" <"$input" >"$log" 2>&1 &
pid=$!

attempt_limit=$timeout_seconds
attempt=0
input_sent=false
while [ "$attempt" -lt "$attempt_limit" ]; do
    if grep -Eq 'HypeR: kernel startup.*failed|HypeR crash monitor' "$log" ||
        grep -Eq '(^|[^[:alnum:]_])(PANIC|BUG)([^[:alnum:]_]|$)' "$log"; then
        cat "$log" >&2
        echo "HypeR reported a fatal failure before Native init completed" >&2
        exit 1
    fi
    # The deferred writer commits a retained opaque Console TX prefix as one
    # frame. Normalize the init runtime's explicit CRLF only; an interleaved
    # kernel record must make the line contract fail rather than be hidden.
    sed 's/\r$//' "$log" >"$native_output"
    if [ "$input_sent" = false ] &&
        grep -Fxq 'HypeR init: console ready' "$native_output"; then
        printf 'HYPER_NATIVE_ECHO_OK\n' >&3
        input_sent=true
    fi
    if grep -q 'HypeR: starting Native init process' "$log" &&
        grep -Fxq 'HypeR init: received input' "$native_output" &&
        grep -Fxq 'HYPER_NATIVE_ECHO_OK' "$native_output"; then
        echo "verified HypeR Native init console echo"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        cat "$log" >&2
        echo "QEMU exited before Native init completed the echo contract" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 1
done

cat "$log" >&2
echo "timed out after ${timeout_seconds}s waiting for Native init console echo" >&2
exit 1
