#!/bin/sh
# Validates the built ELF and raw Linux Image without rebuilding either file.
set -eu

if [ "$#" -ne 5 ]; then
    echo "usage: verify-image.sh LLVM_READOBJ LLVM_NM LLVM_OBJDUMP ELF IMAGE" >&2
    exit 2
fi

readobj=$1
nm=$2
objdump=$3
elf=$4
image=$5

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
printf '%s\n' "$symbols" | grep -q '__aarch64_have_lse_atomics'
printf '%s\n' "$symbols" | grep -q '__aarch64_cas1_acq_rel'

instructions=$($objdump --disassemble "$elf")
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]cas(al|a|l)?b[[:space:]]'
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]ldaxrb[[:space:]]'
printf '%s\n' "$instructions" | grep -Eq '[[:space:]]stl?xrb[[:space:]]'

echo "verified PIE image, RELA/RELR metadata, Linux header, and runtime atomic paths"
