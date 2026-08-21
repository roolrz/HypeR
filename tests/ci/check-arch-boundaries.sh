#!/bin/sh
# Keep target selection at the architecture facade instead of scattering it
# through kernel subsystems and drivers.
set -eu

root=${HYPER_ARCH_BOUNDARY_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

baseline=${HYPER_ARCH_KERNEL_BASELINE:-tests/ci/arch-kernel-dependency-baseline.txt}
raw=$(mktemp "${TMPDIR:-/tmp}/hyper-arch-boundaries.XXXXXX")
current=$(mktemp "${TMPDIR:-/tmp}/hyper-arch-boundaries.XXXXXX")
trap 'rm -f "$raw" "$current"' EXIT HUP INT TERM

matches=$(rg -n --glob '*.rs' 'target_arch' src || true)
violations=$(printf '%s\n' "$matches" | sed '\#^src/arch/#d' | sed '/^$/d')

if [ -n "$violations" ]; then
    echo "target_arch selection is only allowed below src/arch:" >&2
    printf '%s\n' "$violations" >&2
    exit 1
fi

obfuscated=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'use\s+crate\s+as\s+|use\s+crate::kernel\s*(?:;|::\s*\*)|use\s+crate::kernel\s+as\s+|use\s+crate::kernel::(?:crash|irq(?:::(?:interrupt|exception))?|vm(?:::memory)?)\s*(?:;|::\s*\*|\s+as\s+)|use\s+crate::kernel[^;]*::\{|crate::\{[^}]*kernel|crate\s+::\s*kernel|crate::\s+kernel|(?:super::){2,}kernel|#\s*\[\s*path\s*=\s*"[^"]*kernel|include!\s*\(' \
    src/arch || true)
if [ -n "$obfuscated" ]; then
    echo "architecture-to-kernel dependencies must use an explicit crate::kernel path:" >&2
    printf '%s\n' "$obfuscated" >&2
    exit 1
fi

entry_policy_bypass=$(LC_ALL=C rg -n -U --glob '*.rs' \
    'crate::kernel::(?:crash::(?:fatal(?:_context)?|is_stop_interrupt|stop_this_cpu)|irq::(?:acknowledge_external|exception::[A-Za-z_][A-Za-z0-9_]*|interrupt::dispatch)|vm::(?:handle_guest_sync(?:_after_memory_fault)?|memory::resolve_guest_memory_fault))' \
    src/arch || true)
if [ -n "$entry_policy_bypass" ]; then
    echo "architecture exception and VM-exit entry must use the named kernel::entry adapters:" >&2
    printf '%s\n' "$entry_policy_bypass" >&2
    exit 1
fi

architecture_device_owner=$(LC_ALL=C rg -n --glob '*.rs' \
    'static\s+[A-Za-z_][A-Za-z0-9_]*[^;]*(?:VirtualPl011|LegacyPcDevices)|InterruptSpinLock\s*<\s*Option\s*<\s*(?:VirtualPl011|LegacyPcDevices)' \
    src/arch || true)
if [ -n "$architecture_device_owner" ]; then
    echo "mutable virtual-device instances belong to the owning kernel VM aggregate:" >&2
    printf '%s\n' "$architecture_device_owner" >&2
    exit 1
fi

rg_status=0
LC_ALL=C rg -l --glob '*.rs' 'crate::kernel' src/arch >"$raw" || rg_status=$?
if [ "$rg_status" -gt 1 ]; then
    echo "failed to inspect architecture-to-kernel dependencies" >&2
    exit "$rg_status"
fi

: >"$current"
while IFS= read -r source_file; do
    LC_ALL=C rg -o --no-filename \
        'crate::kernel(?:::[A-Za-z_][A-Za-z0-9_]*)*' "$source_file" |
        LC_ALL=C sort |
        uniq -c |
        awk -v source="$source_file" '{ print source "\t" $2 "\t" $1 }' \
            >>"$current"
done <"$raw"
LC_ALL=C sort -k1,1 -k2,2 -o "$current" "$current"

if ! awk -F '\t' '
    FNR == NR {
        if ($0 ~ /^#/ || NF == 0) {
            next
        }
        if (NF != 3 || $1 !~ /^src\/arch\/.*\.rs$/ ||
                $2 !~ /^crate::kernel(::[A-Za-z_][A-Za-z0-9_]*)*$/ ||
                $3 !~ /^[1-9][0-9]*$/) {
            printf "invalid dependency baseline entry at line %d: %s\n", FNR, $0 > "/dev/stderr"
            failed = 1
            next
        }
        key = $1 SUBSEP $2
        if (key in expected) {
            printf "duplicate dependency baseline entry: %s -> %s\n", $1, $2 > "/dev/stderr"
            failed = 1
        }
        expected[key] = $3 + 0
        expected_source[key] = $1
        expected_contract[key] = $2
        next
    }
    {
        key = $1 SUBSEP $2
        observed[key] = $3 + 0
        if (!(key in expected)) {
            printf "new architecture-to-kernel dependency: %s -> %s (%d reference(s))\n", $1, $2, $3 > "/dev/stderr"
            failed = 1
        } else if ($3 > expected[key]) {
            printf "increased architecture-to-kernel dependency: %s -> %s (%d -> %d)\n", $1, $2, expected[key], $3 > "/dev/stderr"
            failed = 1
        } else if ($3 < expected[key]) {
            printf "dependency baseline must be lowered: %s -> %s (%d -> %d)\n", $1, $2, expected[key], $3 > "/dev/stderr"
            failed = 1
        }
    }
    END {
        for (key in expected) {
            if (!(key in observed)) {
                printf "dependency baseline must remove resolved contract: %s -> %s (%d -> 0)\n", expected_source[key], expected_contract[key], expected[key] > "/dev/stderr"
                failed = 1
            }
        }
        exit failed
    }
' "$baseline" "$current"; then
    echo "src/arch must not depend on kernel policy." >&2
    echo "Remove the dependency and lower the temporary baseline in $baseline." >&2
    exit 1
fi
