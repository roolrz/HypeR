#!/bin/sh
# Exercise the IRQ registration ownership checks against deliberate regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-irq-registration-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

source_file=$fixture/interrupt.rs
cp "$root/src/kernel/irq/interrupt.rs" "$source_file"

check() {
    HYPER_IRQ_REGISTRATION_SOURCE="$source_file" \
        sh "$root/tests/ci/check-irq-registration-contract.sh"
}

expect_rejection() {
    description=$1
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

check

sed 's/derive(Debug, Eq, PartialEq)/derive(Clone, Copy, Debug, Eq, PartialEq)/' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "Copy IRQ registration capabilities must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed '/#\[must_use = "an IRQ registration/d' "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "registrations without #[must_use] must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
printf '\nimpl Drop for Registration { fn drop(&mut self) {} }\n' >>"$source_file"
expect_rejection "implicit IRQ unregistration from Drop must be rejected"
