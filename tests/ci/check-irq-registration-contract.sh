#!/bin/sh
# Keep IRQ unregistration an explicit ownership and locking decision.
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

if LC_ALL=C rg -q -U 'impl\s+Drop\s+for\s+Registration' "$source_file"; then
    echo "Registration must not acquire IRQ locks implicitly from Drop" >&2
    exit 1
fi

LC_ALL=C rg -q 'pub fn unregister\(registration: Registration\) -> Result<\(\), UnregisterFailure>' \
    "$source_file" || {
    echo "unregister must consume its capability and return it on failure" >&2
    exit 1
}

LC_ALL=C rg -q 'pub fn retain_permanently\(self\)' "$source_file" || {
    echo "permanent IRQ ownership must remain an explicit conversion" >&2
    exit 1
}
