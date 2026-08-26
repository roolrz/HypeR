#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

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
sed '/^impl Drop for Registration {$/,/^}$/d' "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "registrations without fail-stop Drop must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed '/^impl Drop for Registration {$/,/^}$/ {
    /if self\.armed {/a\
            crate::pr_crit!("unexpected registration drop");
}' "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "Registration Drop diagnostics with hidden locks must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed 's/self\.armed = false;/self.armed = true;/' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "a no-op Registration disarm transition must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed 's/armed: true,/armed: false,/' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "Registration capabilities created unarmed must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed '/^[[:space:]]*registration\.disarm();$/d' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "successful unregister without ownership discharge must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed '/^[[:space:]]*prepared\.disarm();$/d' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "successful prepared discard without ownership discharge must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed 's/Result<(), DiscardFailure>/Result<(), Error>/' "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "discard failure without capability recovery must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed '/^[[:space:]]*reserve_one(&mut domains)?;$/d' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "IRQ state publication without preallocated root storage must be rejected"

cp "$root/src/kernel/irq/interrupt.rs" "$source_file"
sed 's/next_domain: 1,/next_domain: 0,/' \
    "$source_file" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$source_file"
expect_rejection "IRQ publication with a reusable root-domain identity must be rejected"
