// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn counter_deltas_do_not_inherit_recycled_thread_time() {
    let old = ThreadSample {
        process_koid: 1,
        thread_koid: 0x100000001,
        runtime_ticks: 100,
    };
    let current = ThreadSample {
        runtime_ticks: 160,
        ..old
    };
    let recycled = ThreadSample {
        thread_koid: 0x200000001,
        runtime_ticks: 999,
        ..old
    };
    let previous = BTreeMap::from([(old.thread_koid, old)]);
    let samples = BTreeMap::from([
        (current.thread_koid, current),
        (recycled.thread_koid, recycled),
    ]);
    assert_eq!(process_delta(1, &previous, &samples), (60, 2));
    assert_eq!(process_delta(2, &previous, &samples), (0, 0));
}

#[test]
fn busiest_first_with_stable_ties_and_name_sort() {
    let mut rows = vec![
        ProcessSample {
            koid: 3,
            name: "a".into(),
            ticks: 5,
            threads: 1,
        },
        ProcessSample {
            koid: 2,
            name: "z".into(),
            ticks: 50,
            threads: 1,
        },
        ProcessSample {
            koid: 1,
            name: "b".into(),
            ticks: 50,
            threads: 1,
        },
    ];
    sort(&mut rows, Sort::Cpu);
    assert_eq!(rows.iter().map(|p| p.koid).collect::<Vec<_>>(), [1, 2, 3]);
    sort(&mut rows, Sort::Name);
    assert_eq!(rows.iter().map(|p| p.koid).collect::<Vec<_>>(), [3, 1, 2]);
}
