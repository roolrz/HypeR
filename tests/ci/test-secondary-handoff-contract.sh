#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that every late-secondary ownership ratchet rejects its regression.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-secondary-handoff-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/kernel/cpu" "$fixture/src/hal/selected"

check() {
    HYPER_SECONDARY_HANDOFF_ROOT="$fixture" \
        sh "$root/tests/ci/check-secondary-handoff-contract.sh"
}

write_valid_fixture() {
    printf '%s\n' \
        'struct SecondaryBootHandoff {' \
        '    parameters: NonNull<crate::hal::cpu::SecondaryBootParameters>,' \
        '    _block: PageBlock,' \
        '}' \
        'fn place() {' \
        '    let layout = PublicationLayout::new(' \
        '        physical_start,' \
        '        block_start,' \
        '        block_size,' \
        '        parameter_size,' \
        '        align_of::<crate::hal::cpu::SecondaryBootParameters>(),' \
        '        line_size,' \
        '    );' \
        '    let context = layout.physical_address().get();' \
        '    let parameters_address = layout.virtual_address().as_usize().ok_or(Error::InvalidAddress)?;' \
        '    let published_size = layout.published_size();' \
        '    publish_data_range(parameters_address, published_size);' \
        '}' \
        'fn retain() {' \
        '    let records = core::mem::take(&mut self.records);' \
        '    core::mem::forget(records);' \
        '}' \
        'pub fn initialize() {' \
        '    boot_parameters.mark_observable();' \
        '    cpu_on(target);' \
        '    online.load(Ordering::Acquire);' \
        '    boot_parameters.release();' \
        '}' \
        'extern "C" fn enter_clean_idle(cpu_index: CpuIndex) {' \
        '    ONLINE[cpu_index].store(true, Ordering::Release);' \
        '}' \
        >"$fixture/src/kernel/cpu/smp.rs"
    printf '%s\n' \
        'fn validate() {' \
        '    if !valid_page_subdivision(data_line_size()) ||' \
        '        !valid_page_subdivision(instruction_line_size()) {' \
        '        return CacheError::InvalidLineSize;' \
        '    }' \
        '}' \
        'fn valid_page_subdivision(line_size: usize, page_size: usize) -> bool {' \
        '    page_ownership_supports_line(line_size, page_size)' \
        '}' >"$fixture/src/hal/selected/cache.rs"
}

mutate() {
    description=$1
    source=$2
    expression=$3
    write_valid_fixture
    sed "$expression" "$fixture/$source" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

write_valid_fixture
check

write_valid_fixture
printf '%s\n' 'fn bad() { try_box(SecondaryBootParameters::new()); }' \
    >>"$fixture/src/kernel/cpu/smp.rs"
if check >/dev/null 2>&1; then
    echo 'slab-backed secondary handoffs must be rejected' >&2
    exit 1
fi

mutate 'handoffs must retain their dedicated buddy block' \
    src/kernel/cpu/smp.rs \
    's/_block: PageBlock/_block: Box<SecondaryBootParameters>/'
mutate 'handoffs must use the neutral checked publication layout' \
    src/kernel/cpu/smp.rs \
    's/PublicationLayout::new/PublicationLayout::unchecked/'
mutate 'handoffs must align the firmware physical address rather than its virtual alias' \
    src/kernel/cpu/smp.rs \
    's/        physical_start,/        PhysicalAddress::new(block_start.get()),/'
mutate 'firmware and Rust must use aliases from the same layout' \
    src/kernel/cpu/smp.rs \
    's/layout.virtual_address()/block_start/'
mutate 'cache publication must use the complete rounded layout range' \
    src/kernel/cpu/smp.rs \
    's/layout.published_size()/parameter_size/'
mutate 'initialized parameters must be cache-published' \
    src/kernel/cpu/smp.rs \
    's/publish_data_range/publish_later/'
mutate 'firmware observability must be armed before CPU_ON' \
    src/kernel/cpu/smp.rs \
    's/boot_parameters.mark_observable()/boot_parameters.mark_later()/'
mutate 'possibly observed records must remain retained on failure' \
    src/kernel/cpu/smp.rs \
    's/core::mem::forget(records)/drop(records)/'
mutate 'the boot CPU must acquire secondary completion' \
    src/kernel/cpu/smp.rs \
    's/online.load(Ordering::Acquire)/online.load(Ordering::Relaxed)/'
mutate 'the secondary must release-publish handoff consumption' \
    src/kernel/cpu/smp.rs \
    's/ONLINE\[cpu_index\].store(true, Ordering::Release)/ONLINE[cpu_index].store(true, Ordering::Relaxed)/'
mutate 'the all-online path must reclaim retained handoffs' \
    src/kernel/cpu/smp.rs \
    's/boot_parameters.release()/boot_parameters.retain()/'
mutate 'data and instruction line sizes must both be validated' \
    src/hal/selected/cache.rs \
    's/!valid_page_subdivision(instruction_line_size())/false/'
mutate 'selected cache admission must use the neutral ownership proof' \
    src/hal/selected/cache.rs \
    's/page_ownership_supports_line/locally_assume_line/'

write_valid_fixture
sed \
    -e 's/    boot_parameters.mark_observable();/    __OBSERVABILITY_PUBLICATION__;/' \
    -e 's/    cpu_on(target);/    boot_parameters.mark_observable();/' \
    -e 's/    __OBSERVABILITY_PUBLICATION__;/    cpu_on(target);/' \
    "$fixture/src/kernel/cpu/smp.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/cpu/smp.rs"
if check >/dev/null 2>&1; then
    echo 'failure retention must be armed before CPU_ON, not merely present' >&2
    exit 1
fi

write_valid_fixture
sed '/^pub fn initialize(/,/^}/s/boot_parameters.mark_observable()/boot_parameters.mark_later()/' \
    "$fixture/src/kernel/cpu/smp.rs" >"$fixture/mutated"
printf '%s\n' \
    'fn unrelated_decoy() { boot_parameters.mark_observable(); cpu_on(target); }' \
    >>"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/cpu/smp.rs"
if check >/dev/null 2>&1; then
    echo 'failure retention must be checked inside CPU initialization' >&2
    exit 1
fi
