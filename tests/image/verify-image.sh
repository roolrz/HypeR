#!/bin/sh
# Validates the built ELF and raw Linux Image without rebuilding either file.
set -eu

if [ "$#" -ne 6 ]; then
    echo "usage: verify-image.sh LLVM_READOBJ LLVM_NM LLVM_OBJDUMP ELF IMAGE KALLSYMS" >&2
    exit 2
fi

readobj=$1
nm=$2
objdump=$3
elf=$4
image=$5
kallsyms=$6

header=$($readobj --file-headers "$elf")
printf '%s\n' "$header" | grep -q 'Type: SharedObject'
printf '%s\n' "$header" | grep -q 'Machine: EM_AARCH64'

relocation_types=$($readobj --relocations "$elf" \
    | sed -n 's/.*\(R_AARCH64_[A-Z0-9_]*\).*/\1/p' \
    | sort -u)
if [ "$relocation_types" != "R_AARCH64_RELATIVE" ]; then
    echo "unsupported dynamic relocation set: $relocation_types" >&2
    exit 1
fi

sections=$($readobj --sections "$elf")
printf '%s\n' "$sections" | grep -q 'Name: .relr.dyn'
printf '%s\n' "$sections" | grep -q 'Type: SHT_RELR'
dynamic=$($readobj --dynamic-table "$elf")
printf '%s\n' "$dynamic" | grep -q ' RELR '
printf '%s\n' "$dynamic" | grep -q ' RELRSZ '
printf '%s\n' "$dynamic" | grep -q ' RELRENT '
printf '%s\n' "$sections" | grep -q 'Name: .dynsym'
printf '%s\n' "$sections" | grep -q 'Type: SHT_DYNSYM'
printf '%s\n' "$sections" | grep -q 'Name: .dynstr'

magic=$(dd if="$image" bs=1 skip=56 count=4 2>/dev/null | od -An -tx1 | tr -d ' \n')
if [ "$magic" != "41524d64" ]; then
    echo "invalid Linux AArch64 Image magic: $magic" >&2
    exit 1
fi

declared_size=$(od -An -tu8 -j 16 -N 8 "$image" | tr -d ' ')
actual_size=$(wc -c < "$image" | tr -d ' ')
if [ "$actual_size" -gt "$declared_size" ]; then
    echo "Image payload exceeds its declared memory footprint" >&2
    exit 1
fi
if [ $((declared_size % 4096)) -ne 0 ]; then
    echo "Image memory footprint is not page aligned: $declared_size" >&2
    exit 1
fi

symbols=$($nm -a "$elf")
printf '%s\n' "$symbols" | grep -q '__relr_dyn_start'
printf '%s\n' "$symbols" | grep -q '__relr_dyn_end'
printf '%s\n' "$symbols" | grep -q '__kallsyms_symbols_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_symbols_end'
printf '%s\n' "$symbols" | grep -q '__kallsyms_strings_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_strings_end'
printf '%s\n' "$symbols" | grep -q '__kallsyms_start'
printf '%s\n' "$symbols" | grep -q '__kallsyms_end'
printf '%s\n' "$symbols" | grep -q '__aarch64_have_lse_atomics'
printf '%s\n' "$symbols" | grep -q '__aarch64_cas1_acq_rel'

instructions=$($objdump --disassemble "$elf")
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]cas(al|a|l)?b[[:space:]]'
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]ldaxrb[[:space:]]'
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]stl?xrb[[:space:]]'

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

echo "verified PIE, RELA/RELR, kallsyms, Linux header, and runtime atomic paths"
