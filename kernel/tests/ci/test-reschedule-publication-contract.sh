#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that every reschedule-publication ordering ratchet is falsifiable.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-reschedule-publication-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/kernel/task"
source=$fixture/src/kernel/task/reschedule.rs

check() {
    HYPER_RESCHEDULE_PUBLICATION_ROOT="$fixture" \
        sh "$root/tests/ci/check-reschedule-publication-contract.sh"
}

write_valid_fixture() {
    printf '%s\n' \
        'struct Pending(AtomicBool);' \
        'impl Pending {' \
        '    pub fn publish(&self) -> bool {' \
        '        !self.0.swap(true, Ordering::Release)' \
        '    }' \
        '    pub fn is_pending(&self) -> bool {' \
        '        self.0.load(Ordering::Acquire)' \
        '    }' \
        '    pub fn take(&self) -> bool {' \
        '        self.0.swap(false, Ordering::AcqRel)' \
        '    }' \
        '}' >"$source"
}

mutate() {
    description=$1
    expression=$2
    write_valid_fixture
    sed "$expression" "$source" >"$fixture/mutated"
    mv "$fixture/mutated" "$source"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

write_valid_fixture
check

mutate 'Relaxed reschedule publication must be rejected' \
    's/swap(true, Ordering::Release)/swap(true, Ordering::Relaxed)/'
mutate 'Relaxed pending observation must be rejected' \
    's/load(Ordering::Acquire)/load(Ordering::Relaxed)/'
mutate 'Acquire-only consumption must be rejected' \
    's/swap(false, Ordering::AcqRel)/swap(false, Ordering::Acquire)/'

write_valid_fixture
awk '
    /!self\.0\.swap\(true, Ordering::Release\)/ {
        print "        // !self.0.swap(true, Ordering::Release)"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$source" >"$fixture/mutated"
mv "$fixture/mutated" "$source"
if check >/dev/null 2>&1; then
    echo 'comment-only ordering expressions must not satisfy the publication contract' >&2
    exit 1
fi

write_valid_fixture
awk '
    /!self\.0\.swap\(true, Ordering::Release\)/ {
        print "        /* !self.0.swap(true, Ordering::Release) */"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$source" >"$fixture/mutated"
mv "$fixture/mutated" "$source"
if check >/dev/null 2>&1; then
    echo 'block-comment ordering expressions must not satisfy the publication contract' >&2
    exit 1
fi
