#!/bin/sh
# Keep allocation-free firmware discovery within every bootstrap stack.
set -eu

root=${HYPER_BOOT_STACK_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

minimum_kib=256
minimum_pages=64

for linker in \
    src/arch/aarch64/linker.ld \
    src/arch/riscv64/linker.ld \
    src/arch/x86_64/linker.ld; do
    declarations=$(sed -n 's/^BOOT_STACK_SIZE = \([0-9][0-9]*\)K;$/\1/p' "$linker")
    if [ "$(printf '%s\n' "$declarations" | sed '/^$/d' | wc -l | tr -d ' ')" -ne 1 ]; then
        echo "$linker must define BOOT_STACK_SIZE exactly once" >&2
        exit 1
    fi
    size_kib=$declarations
    if [ -z "$size_kib" ] || [ "$size_kib" -lt "$minimum_kib" ]; then
        echo "$linker must reserve at least ${minimum_kib} KiB for bounded boot discovery" >&2
        exit 1
    fi
done

check_final_stack_pages() {
    source_file=$1
    constant=$2
    declarations=$(LC_ALL=C rg --no-line-number --only-matching \
        "^[[:space:]]*(?:pub(?:\\([^)]*\\))?[[:space:]]+)?const[[:space:]]+${constant}[[:space:]]*:[[:space:]]*usize[[:space:]]*=[[:space:]]*[0-9]+[[:space:]]*;" \
        "$source_file" || true)
    if [ "$(printf '%s\n' "$declarations" | sed '/^$/d' | wc -l | tr -d ' ')" -ne 1 ]; then
        echo "$source_file must define $constant exactly once" >&2
        exit 1
    fi
    pages=$(printf '%s\n' "$declarations" | sed -n 's/^.*=[[:space:]]*\([0-9][0-9]*\)[[:space:]]*;$/\1/p')
    if [ -z "$pages" ] || [ "$pages" -lt "$minimum_pages" ]; then
        echo "$source_file must retain at least $minimum_pages pages for final boot discovery" >&2
        exit 1
    fi
}

check_final_stack_pages src/arch/aarch64/memory/page_table.rs KERNEL_STACK_PAGES
check_final_stack_pages src/arch/riscv64/memory/page_table.rs KERNEL_STACK_PAGES
check_final_stack_pages src/arch/x86_64/memory.rs STACK_PAGES
