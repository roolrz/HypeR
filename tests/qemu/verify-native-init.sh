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
timeout_seconds=${QEMU_BOOT_TIMEOUT_SECONDS:-90}
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
command_phase='console'
while [ "$attempt" -lt "$attempt_limit" ]; do
    if grep -Eq 'HypeR: kernel startup.*failed|HypeR crash monitor' "$log" ||
        grep -Eq '(^|[^[:alnum:]_])(PANIC|BUG)([^[:alnum:]_]|$)' "$log" ||
        grep -Eq 'HypeR init: (critical service |initial VM (failed|protocol failed)|VM (manager terminated|provisioning channel closed))' "$log"; then
        cat "$log" >&2
        echo "HypeR reported a fatal failure before Native init completed" >&2
        exit 1
    fi
    # The deferred writer commits a retained opaque Console TX prefix as one
    # frame. Normalize an application's explicit CRLF only; an interleaved
    # kernel record must make the line contract fail rather than be hidden.
    sed 's/\r$//' "$log" >"$native_output"
    if grep -Eq '^(hyper-sh\$ )*sh: (Native operation failed|I/O failed|input channel closed|protocol violation|command launch failed|operating-system request failed)' "$native_output"; then
        cat "$log" >&2
        echo "Native shell reported an unexpected runtime failure" >&2
        exit 1
    fi
    case "$command_phase" in
        console)
            if grep -Fxq 'HypeR session: console ready' "$native_output"; then
                # Empty lines and repeated terminal DEL/BS must not prefix the
                # first command with an invisible byte.
                printf '\r\n\r\n\177\177\010vmm --help\r' >&3
                command_phase='first_command'
            fi
            ;;
        first_command)
            if grep -q '^Usage: vmm' "$native_output"; then
                command_phase='vm_running'
            fi
            ;;
        vm_running)
            if grep -q 'HypeR: vCPU 0 running as scheduler thread' "$log"; then
                printf '/bin/vmm console\n' >&3
                command_phase='guest_console'
            fi
            ;;
        guest_console)
            if grep -q 'HypeR guest: repeated timer wakeups passed' "$log" &&
                grep -Fq '~ # ' "$log"; then
                # No Enter yet: a line-buffered relay must not hide guest echo.
                printf 'echo HYPER_GUEST_CONSOLE_RX' >&3
                command_phase='guest_echo'
            fi
            ;;
        guest_echo)
            if grep -Fq 'echo HYPER_GUEST_CONSOLE_RX' "$log"; then
                printf '\r' >&3
                command_phase='guest_enter'
            fi
            ;;
        guest_enter)
            # Check the raw stream: the Linux tty owns CR/LF conversion.
            if grep -Fxq "$(printf 'HYPER_GUEST_CONSOLE_RX\r')" "$log"; then
                printf '\035d' >&3
                command_phase='guest_detach'
            fi
            ;;
        guest_detach)
            if grep -Fxq '[vmm] detached' "$native_output"; then
                printf '/bin/ps\n' >&3
                command_phase='ps'
            fi
            ;;
        ps)
            if grep -Eq '^process  [[:space:]]*[0-9]+[[:space:]]+-[[:space:]]+[^[:space:]]+[[:space:]]+(created|running|stopping|stopped|retiring|retired)' "$native_output"; then
                printf '/bin/ps --threads\n' >&3
                command_phase='threads'
            fi
            ;;
        threads)
            if grep -Eq '^  thread [[:space:]]*[0-9]+[[:space:]]+[0-9]+[[:space:]]+[^[:space:]]+[[:space:]]+user/(resident|retiring)' "$native_output" &&
                grep -Eq '^  thread [[:space:]]*[0-9]+[[:space:]]+-[[:space:]]+[^[:space:]]+[[:space:]]+(bootstrap|idle|kernel|vcpu)/(resident|retiring)' "$native_output"; then
                printf '/bin/echo HYPER_NATIVE_PS_OK\n' >&3
                command_phase='ps_done'
            fi
            ;;
        ps_done)
            if grep -Fxq 'HYPER_NATIVE_PS_OK' "$native_output"; then
                process_koid=$(awk '/^process  [[:space:]]*[0-9]+/ && $4 == "shell" { print $2; exit }' "$native_output")
                if [ -z "$process_koid" ]; then
                    echo "could not select a persistent Process observation" >&2
                    exit 1
                fi
                printf '/bin/handle %s\n' "$process_koid" >&3
                command_phase='handles'
            fi
            ;;
        handles)
            if grep -Eq '^0x[0-9a-f]+[[:space:]]+[0-9]+[[:space:]]+[a-z][a-z-]+[[:space:]]+[a-z][a-z|-]+' "$native_output"; then
                printf '/bin/handle --objects\n' >&3
                command_phase='objects'
            fi
            ;;
        objects)
            if grep -Eq '^[0-9]+[[:space:]]+[a-z][a-z-]+[[:space:]]+(unpublished|active|retired)' "$native_output"; then
                printf '/bin/dynamic-test\n' >&3
                command_phase='dynamic'
            fi
            ;;
        dynamic)
            if grep -Fxq 'HYPER_DYNAMIC_LINK_OK' "$native_output"; then
                printf '/bin/echo-static HYPER_STATIC_LINK_OK\n' >&3
                command_phase='static'
            fi
            ;;
        static)
            if grep -Fxq 'HYPER_STATIC_LINK_OK' "$native_output"; then
                printf '/bin/std-test --name dynamic --read-input\n' >&3
                command_phase='std_input'
            fi
            ;;
        std_input)
            if grep -Fxq 'HYPER_STD_INPUT_READY' "$native_output"; then
                printf 'abcdef\n' >&3
                command_phase='std_dynamic'
            fi
            ;;
        std_dynamic)
            if grep -Fxq 'HYPER_STD_OK hello dynamic' "$native_output" &&
                grep -Fxq 'HYPER_STD_TLS_DROP_OK' "$native_output"; then
                printf '/bin/std-test-static --name static\n' >&3
                command_phase='std_static'
            fi
            ;;
        std_static)
            if grep -Fxq 'HYPER_STD_OK hello static' "$native_output"; then
                printf '/bin/std-test --help\n' >&3
                command_phase='std_help'
            fi
            ;;
        std_help)
            if grep -Fxq 'HypeR standard library acceptance probe' "$native_output" &&
                grep -q '^Usage: .*std-test' "$native_output"; then
                if grep -Eq '^(hyper-sh\$ )*sh: command failed$' "$native_output"; then
                    cat "$log" >&2
                    echo "std success/help path returned failure" >&2
                    exit 1
                fi
                printf '/bin/std-test --unknown-option\n' >&3
                command_phase='std_error'
            fi
            ;;
        std_error)
            # stdout and stderr use separate channels: the next shell prompt
            # may arrive between fragments of this stderr diagnostic.
            if sed 's/hyper-sh\$ //g' "$native_output" |
                grep -q "unexpected argument '--unknown-option'" &&
                grep -Eq '^(hyper-sh\$ )*sh: command failed$' "$native_output"; then
                printf '/bin/std-test --panic\n' >&3
                command_phase='std_panic'
            fi
            ;;
        std_panic)
            if grep -q 'HYPER_STD_EXPECTED_PANIC' "$native_output" &&
                [ "$(grep -Ec '^(hyper-sh\$ )*sh: command failed$' "$native_output")" -eq 2 ]; then
                printf 'ps --help\n' >&3
                command_phase='cli_ps'
            fi
            ;;
        cli_ps)
            if grep -Fq 'List Native processes and threads' "$native_output"; then
                printf 'handle --help\n' >&3
                command_phase='cli_handle'
            fi
            ;;
        cli_handle)
            if grep -Fq 'Inspect Native kernel objects or a process' "$native_output"; then
                printf 'ls --help\n' >&3
                command_phase='cli_ls'
            fi
            ;;
        cli_ls)
            if grep -Fq 'List a delegated directory' "$native_output"; then
                printf 'free --help\n' >&3
                command_phase='cli_free'
            fi
            ;;
        cli_free)
            if grep -Fq 'Display physical memory usage' "$native_output"; then
                printf 'top --help\n' >&3
                command_phase='cli_top'
            fi
            ;;
        cli_top)
            if grep -Fq 'Monitor Native CPU, memory and processes' "$native_output"; then
                printf 'vmm --help\n' >&3
                command_phase='cli_vmm'
            fi
            ;;
        cli_vmm)
            if grep -Fq 'Manage the default virtual machine' "$native_output"; then
                printf 'sh --help\n' >&3
                command_phase='cli_shell'
            fi
            ;;
        cli_shell)
            if grep -Fq 'Native capability-scoped command shell' "$native_output"; then
                printf 'cd --help\n' >&3
                command_phase='cli_builtin'
            fi
            ;;
        cli_builtin)
            if grep -Fq 'Usage: sh cd' "$native_output"; then
                printf 'echo HYPER_CLAP_BUILTIN_OK\n' >&3
                command_phase='cli_echo'
            fi
            ;;
        cli_echo)
            if grep -Fq 'HYPER_CLAP_BUILTIN_OK' "$native_output"; then
                printf 'ls\n' >&3
                command_phase='ls_root'
            fi
            ;;
        ls_root)
            if grep -Fxq 'bin/' "$native_output" &&
                grep -Fxq 'etc/' "$native_output" &&
                grep -Fxq 'lib/' "$native_output"; then
                printf 'cd /bin\npwd\nls\n' >&3
                command_phase='ls_bin'
            fi
            ;;
        ls_bin)
            if grep -Fxq 'echo' "$native_output" &&
                grep -Fxq 'ls' "$native_output" &&
                grep -Fxq 'sh' "$native_output" &&
                grep -Fxq '/bin' "$native_output"; then
                printf './echo HYPER_CD_CHILD_OK\n' >&3
                command_phase='cd_child'
            fi
            ;;
        cd_child)
            if grep -Fxq 'HYPER_CD_CHILD_OK' "$native_output"; then
                printf 'cd ..\npwd\nbin/echo HYPER_CD_PARENT_OK\n' >&3
                command_phase='cd_parent'
            fi
            ;;
        cd_parent)
            if grep -Fxq 'HYPER_CD_PARENT_OK' "$native_output"; then
                printf '/bin/free\n' >&3
                command_phase='free'
            fi
            ;;
        free)
            if grep -Eq '^Mem:[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB$' "$native_output" &&
                grep -Eq '^Owners:[[:space:]]+kernel=[0-9]+ MiB heap=[0-9]+ MiB tables=[0-9]+ MiB user=[0-9]+ MiB guest=[0-9]+ MiB other=[0-9]+ MiB$' "$native_output"; then
                printf '/bin/top\n' >&3
                command_phase='top'
            fi
            ;;
        top)
            if grep -Eq 'top - [0-9]+ CPUs[[:space:]]+ticks=[0-9]+ Hz' "$native_output" &&
                grep -Fxq 'Press q to quit.' "$native_output"; then
                printf 'q' >&3
                command_phase='top_exit'
            fi
            ;;
        top_exit)
            if awk '/^Press q to quit[.]$/ { seen = 1; ready = 0; next }
                seen && /^hyper-sh\$ q?$/ { ready = 1 }
                END { exit !ready }' "$native_output"; then
                printf '/bin/echo HYPER_NATIVE_ECHO_OK\n' >&3
                command_phase='echo'
            fi
            ;;
        echo)
            # Wait for this command's prompt, not any earlier prompt in the log.
            if awk '/^HYPER_NATIVE_ECHO_OK$/ { seen = 1; next }
                seen && /^hyper-sh\$ $/ { ready = 1 }
                END { exit !ready }' "$native_output"; then
                command_phase='echo_done'
            fi
            ;;
        echo_done) ;;

    esac
    if [ "$command_phase" = echo_done ] &&
        grep -q 'HypeR: starting Native init process' "$log" &&
        grep -Fxq 'HypeR session: console ready' "$native_output" &&
        grep -Fxq 'TYPE     KOID       OWNER      NAME                 STATE' "$native_output" &&
        grep -Fxq 'HANDLE             OBJECT     KIND                    RIGHTS                           PURPOSE' "$native_output" &&
        grep -Fxq 'KOID       KIND                    HANDLE-STATE HANDLES REFS PURPOSE' "$native_output" &&
        grep -Fxq 'HYPER_DYNAMIC_LINK_OK' "$native_output" &&
        grep -Fxq 'HYPER_STATIC_LINK_OK' "$native_output" &&
        grep -Fxq 'HYPER_STD_OK hello dynamic' "$native_output" &&
        grep -Fxq 'HYPER_STD_OK hello static' "$native_output" &&
        [ "$(grep -Fxc 'HYPER_STD_TLS_DROP_OK' "$native_output")" -eq 2 ] &&
        [ "$(grep -Ec '^(hyper-sh\$ )*HYPER_STD_STDERR_OK$' "$native_output")" -eq 2 ] &&
        grep -Fxq '/bin' "$native_output" &&
        grep -Fxq '/' "$native_output" &&
        grep -Fxq 'HYPER_CLAP_BUILTIN_OK' "$native_output" &&
        grep -Fxq 'HYPER_CD_CHILD_OK' "$native_output" &&
        grep -Fxq 'HYPER_CD_PARENT_OK' "$native_output" &&
        grep -Eq '^Mem:[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB[[:space:]]+[0-9]+ MiB' "$native_output" &&
        grep -Fxq 'Press q to quit.' "$native_output" &&
        grep -Fxq 'HYPER_NATIVE_ECHO_OK' "$native_output" &&
        grep -q 'Run /init as init process' "$log" &&
        grep -q 'HypeR guest: /init reached' "$log" &&
        grep -q 'HypeR guest: repeated timer wakeups passed' "$log"; then
        echo "verified Native services and userspace-managed Linux VM startup"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        cat "$log" >&2
        echo "QEMU exited before Native init completed the inspection contract" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 1
done

cat "$log" >&2
echo "timed out after ${timeout_seconds}s waiting for Native init inspection" >&2
exit 1
