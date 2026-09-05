#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep IRQ registration ownership linear without hiding lock acquisition in Drop.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
source_file=${HYPER_IRQ_REGISTRATION_SOURCE:-$root/src/kernel/irq/interrupt.rs}

if ! awk '
    /^#\[must_use = / {
        must_use = 1
        next
    }
    must_use && /^#\[derive\(/ {
        derive = $0
        next
    }
    /^pub struct Registration / {
        found = 1
        if (!must_use || derive ~ /(^|[ (,])(Clone|Copy)([ ,)]|$)/) {
            invalid = 1
        }
        exit
    }
    must_use && $0 !~ /^\/\// && $0 !~ /^#\[/ {
        must_use = 0
        derive = ""
    }
    END {
        exit (!found || invalid)
    }
' "$source_file"; then
    echo "Registration must be #[must_use] and must not implement Clone or Copy" >&2
    exit 1
fi

if ! LC_ALL=C rg -q -U \
    'pub struct Registration\s*\{[^}]*armed:\s*bool' "$source_file"; then
    echo "Registration must retain explicit armed ownership state" >&2
    exit 1
fi

drop_body=$(sed -n '/^impl Drop for Registration {$/,/^}$/p' "$source_file")
if [ -z "$drop_body" ] ||
    ! printf '%s\n' "$drop_body" | LC_ALL=C rg -q 'if self\.armed' ||
    ! printf '%s\n' "$drop_body" | LC_ALL=C rg -q 'crate::hal::cpu::halt\(\)' ||
    printf '%s\n' "$drop_body" | LC_ALL=C rg -q \
        'unregister|with_state|INTERRUPTS|controller|disable|remove_prepared|pr_|print|log::|crash::'; then
    echo "armed Registration Drop must fail-stop without locks or hardware operations" >&2
    exit 1
fi

LC_ALL=C rg -q -U \
    'const fn new\([^)]*\)[^{]*\{[^}]*armed:\s*true,' "$source_file" || {
    echo "new Registration capabilities must begin armed" >&2
    exit 1
}

LC_ALL=C rg -q -U \
    'fn disarm\(&mut self\)\s*\{\s*if !self\.armed\s*\{\s*crate::hal::cpu::halt\(\)[[:space:]]*\}[[:space:]]*self\.armed\s*=\s*false;' \
    "$source_file" || {
    echo "Registration must have one checked disarm transition" >&2
    exit 1
}

LC_ALL=C rg -q 'pub fn unregister\(mut registration: Registration\) -> Result<\(\), UnregisterFailure>' \
    "$source_file" || {
    echo "unregister must consume its capability and return it on failure" >&2
    exit 1
}

unregister_body=$(sed -n '/^pub fn unregister(mut registration: Registration)/,/^}$/p' "$source_file")
printf '%s\n' "$unregister_body" | LC_ALL=C rg -q -U \
    'registration\.disarm\(\);[[:space:]]*Ok\(\(\)\)' || {
    echo "successful unregister must disarm ownership exactly before returning" >&2
    exit 1
}

LC_ALL=C rg -q 'pub fn discard_prepared\(prepared: PreparedRegistration\) -> Result<\(\), DiscardFailure>' \
    "$source_file" || LC_ALL=C rg -q \
    'pub fn discard_prepared\(mut prepared: PreparedRegistration\) -> Result<\(\), DiscardFailure>' \
    "$source_file" || {
    echo "discard_prepared must return its exclusive capability on failure" >&2
    exit 1
}

discard_body=$(sed -n '/^pub fn discard_prepared(mut prepared: PreparedRegistration)/,/^}$/p' "$source_file")
printf '%s\n' "$discard_body" | LC_ALL=C rg -q 'prepared\.disarm\(\);' || {
    echo "successful prepared-mapping discard must disarm ownership" >&2
    exit 1
}

prepare_body=$(sed -n '/^    pub fn prepare_shared_mapping(/,/^    }$/p' "$source_file")
if ! printf '%s\n' "$prepare_body" | LC_ALL=C rg -q \
        'remove_prepared_mapping\(&prepared\)' ||
    ! printf '%s\n' "$prepare_body" | LC_ALL=C rg -q \
        'prepared\.disarm\(\);'; then
    echo "failed prepared configuration must remove and disarm its local capability" >&2
    exit 1
fi

LC_ALL=C rg -q -U 'pub struct DiscardFailure \{[^}]*prepared: PreparedRegistration' \
    "$source_file" || {
    echo "DiscardFailure must preserve the prepared mapping capability" >&2
    exit 1
}

LC_ALL=C rg -q -U \
    'pub fn retain_permanently\(mut self\)\s*\{\s*self\.disarm\(\);' "$source_file" || {
    echo "permanent IRQ ownership must remain an explicit conversion" >&2
    exit 1
}

initialize_body=$(sed -n '/^pub fn initialize(info: InterruptControllerInfo)/,/^}$/p' "$source_file")
if ! printf '%s\n' "$initialize_body" | LC_ALL=C rg -q \
        'let root_domain = IrqDomainId\(0\);' ||
    ! printf '%s\n' "$initialize_body" | LC_ALL=C rg -q -U \
        'reserve_one\(&mut domains\)\?;' ||
    ! printf '%s\n' "$initialize_body" | LC_ALL=C rg -q -U \
        'domains,[[:space:]]*next_domain: 1,' ||
    printf '%s\n' "$initialize_body" | LC_ALL=C rg -q 'create_domain\(\)\?'; then
    echo "IRQ initialization must construct its root domain before one-shot publication" >&2
    exit 1
fi
