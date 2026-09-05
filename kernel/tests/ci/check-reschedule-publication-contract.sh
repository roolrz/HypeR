#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the architecture-neutral reschedule publication protocol.
set -eu

root=${HYPER_RESCHEDULE_PUBLICATION_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

source=src/kernel/task/reschedule.rs

method_body() {
    method=$1
    sed -n "/^    pub fn $method/,/^    }/p" "$source" |
        sed '\#^[[:space:]]*//#d'
}

require_method() {
    method=$1
    pattern=$2
    message=$3
    body=$(method_body "$method")
    if [ -z "$body" ] || ! printf '%s\n' "$body" | rg -q "$pattern"; then
        echo "$message" >&2
        exit 1
    fi
}

require_method_occurrences() {
    method=$1
    pattern=$2
    expected=$3
    message=$4
    count=$(method_body "$method" | rg -o "$pattern" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -ne "$expected" ]; then
        echo "$message (expected $expected, found $count)" >&2
        exit 1
    fi
}

reject_commented_contract() {
    method=$1
    raw_body=$(sed -n "/^    pub fn $method/,/^    }/p" "$source")
    if printf '%s\n' "$raw_body" | rg -q -U \
        '(?s)/\*.*?(self\.0\.(swap|load)|Ordering::(Acquire|Release|AcqRel)).*?\*/'; then
        echo "commented-out code must not satisfy PendingReschedule::$method" >&2
        exit 1
    fi
}

require_method \
    'publish(' \
    '^[[:space:]]*!self\.0\.swap\(true, Ordering::Release\)' \
    'reschedule publication must release all preceding scheduler state'
require_method \
    'is_pending(' \
    '^[[:space:]]*self\.0\.load\(Ordering::Acquire\)' \
    'reschedule observation must acquire published scheduler state'
require_method \
    'take(' \
    '^[[:space:]]*self\.0\.swap\(false, Ordering::AcqRel\)' \
    'reschedule consumption must acquire publication and order the next epoch'

require_method_occurrences 'publish(' 'self\.0\.swap\(' 1 \
    'reschedule publication must have exactly one atomic transition'
require_method_occurrences 'is_pending(' 'self\.0\.load\(' 1 \
    'reschedule observation must have exactly one atomic load'
require_method_occurrences 'take(' 'self\.0\.swap\(' 1 \
    'reschedule consumption must have exactly one atomic transition'
reject_commented_contract 'publish('
reject_commented_contract 'is_pending('
reject_commented_contract 'take('
