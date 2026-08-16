#!/bin/sh
# Validates the built ELF and raw Linux Image without rebuilding either file.
set -eu

if [ "$#" -ne 7 ]; then
    echo "usage: verify-image.sh ARCH LLVM_READOBJ LLVM_NM LLVM_OBJDUMP ELF IMAGE KALLSYMS" >&2
    exit 2
fi

arch=$1
readobj=$2
nm=$3
objdump=$4
elf=$5
image=$6
kallsyms=$7

case "$arch" in
    aarch64)
        machine=EM_AARCH64
        relative=R_AARCH64_RELATIVE
        ;;
    riscv64)
        machine=EM_RISCV
        relative=R_RISCV_RELATIVE
        ;;
    x86_64)
        machine=EM_X86_64
        relative=R_X86_64_RELATIVE
        ;;
    *)
        echo "unsupported image architecture: $arch" >&2
        exit 2
        ;;
esac

header=$($readobj --file-headers "$elf")
printf '%s\n' "$header" | grep -q 'Type: SharedObject'
printf '%s\n' "$header" | grep -q "Machine: $machine"

relocation_types=$($readobj --relocations "$elf" \
    | sed -n 's/.*\(R_[A-Z0-9_]*\).*/\1/p' \
    | sort -u)
if [ "$relocation_types" != "$relative" ]; then
    echo "unsupported dynamic relocation set: $relocation_types" >&2
    exit 1
fi

sections=$($readobj --sections "$elf")
dynamic=$($readobj --dynamic-table "$elf")
if [ "$arch" != riscv64 ]; then
    printf '%s\n' "$sections" | grep -q 'Name: .relr.dyn'
    printf '%s\n' "$sections" | grep -q 'Type: SHT_RELR'
    printf '%s\n' "$dynamic" | grep -q ' RELR '
    printf '%s\n' "$dynamic" | grep -q ' RELRSZ '
    printf '%s\n' "$dynamic" | grep -q ' RELRENT '
fi
printf '%s\n' "$sections" | grep -q 'Name: .dynsym'
printf '%s\n' "$sections" | grep -q 'Type: SHT_DYNSYM'
printf '%s\n' "$sections" | grep -q 'Name: .dynstr'

case "$arch" in
    aarch64|riscv64)
        magic=$(dd if="$image" bs=1 skip=56 count=4 2>/dev/null | od -An -tx1 | tr -d ' \n')
        expected_magic=41524d64
        [ "$arch" = riscv64 ] && expected_magic=52534305
        if [ "$magic" != "$expected_magic" ]; then
            echo "invalid Linux $arch Image magic: $magic" >&2
            exit 1
        fi

        declared_size=$(od -An -tu8 -j 16 -N 8 "$image" | tr -d ' ')
        actual_size=$(wc -c < "$image" | tr -d ' ')
        if [ "$actual_size" -gt "$declared_size" ]; then
            echo "Image payload exceeds its declared memory footprint" >&2
            exit 1
        fi
        if [ "$arch" = aarch64 ] && [ $((declared_size % 4096)) -ne 0 ]; then
            echo "Image memory footprint is not page aligned: $declared_size" >&2
            exit 1
        fi
        ;;
    x86_64)
        boot_flag=$(dd if="$image" bs=1 skip=510 count=2 2>/dev/null | od -An -tx1 | tr -d ' \n')
        header_magic=$(dd if="$image" bs=1 skip=514 count=4 2>/dev/null)
        setup_sectors=$(od -An -tu1 -j 497 -N 1 "$image" | tr -d ' ')
        actual_size=$(wc -c < "$image" | tr -d ' ')
        if [ "$boot_flag" != 55aa ] || [ "$header_magic" != HdrS ]; then
            echo "invalid Linux x86 setup header" >&2
            exit 1
        fi
        if [ "$setup_sectors" -ne 4 ] || [ "$actual_size" -le 2560 ]; then
            echo "invalid Linux x86 protected payload offset" >&2
            exit 1
        fi
        ;;
esac

symbols=$($nm -a "$elf")
if [ "$arch" != riscv64 ]; then
    printf '%s\n' "$symbols" | grep -q '__relr_dyn_start'
    printf '%s\n' "$symbols" | grep -q '__relr_dyn_end'
fi
printf '%s\n' "$symbols" | grep -q '__kallsyms_symbols_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_symbols_end'
printf '%s\n' "$symbols" | grep -q '__kallsyms_strings_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_strings_end'
printf '%s\n' "$symbols" | grep -q '__kallsyms_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_end'

instructions=$($objdump --disassemble "$elf")
case "$arch" in
    aarch64)
        printf '%s\n' "$symbols" | grep -q '__aarch64_have_lse_atomics'
        printf '%s\n' "$symbols" | grep -q '__aarch64_cas1_acq_rel'
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]cas(al|a|l)?b[[:space:]]'
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]ldaxrb[[:space:]]'
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]stl?xrb[[:space:]]'
        ;;
    riscv64)
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]amo(add|swap)\.d'
        ;;
    x86_64)
        printf '%s\n' "$symbols" | grep -q ' x86_64_protected_entry$'
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]lock$'
        printf '%s\n' "$instructions" | grep -Eq '[[:space:]]vm(launch|resume)$'
        ;;
esac

dynamic_symbols=$($nm -D --defined-only "$elf")
printf '%s\n' "$dynamic_symbols" | grep -q ' hyper_kallsyms_lookup$'
printf '%s\n' "$sections" | grep -q 'Name: \.kallsyms'

kallsyms_section_size=$(printf '%s\n' "$sections" | awk '
    /Name: \.kallsyms / { in_kallsyms = 1; next }
    in_kallsyms && /Size:/ { print $2; exit }
')
kallsyms_file_size=$(wc -c < "$kallsyms" | tr -d ' ')
kallsyms_strings_offset=$(od -An -tu4 -j 16 -N 4 "$kallsyms" | tr -d ' ')
kallsyms_strings_size=$(od -An -tu4 -j 20 -N 4 "$kallsyms" | tr -d ' ')
kallsyms_used=$((kallsyms_strings_offset + kallsyms_strings_size))
if [ "$kallsyms_section_size" -ne "$kallsyms_file_size" ] ||
    [ "$kallsyms_used" -ne "$kallsyms_file_size" ]; then
    echo "kallsyms section is not exact-sized: section=$kallsyms_section_size, used=$kallsyms_used, file=$kallsyms_file_size" >&2
    exit 1
fi

echo "verified $arch PIE relocations, kallsyms, Linux header, and runtime instruction paths"
