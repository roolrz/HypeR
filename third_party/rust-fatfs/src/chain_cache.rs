// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded seek checkpoints owned by one mounted filesystem. Appending a
//! cluster preserves existing positions; freeing or truncating a chain must
//! invalidate every checkpoint before changing the FAT, including on failure.

const RUNS: usize = 16;

#[derive(Clone, Copy)]
struct Run {
    first: u32,
    logical: u32,
    physical: u32,
    count: u32,
}

impl Run {
    const EMPTY: Self = Self {
        first: 0,
        logical: 0,
        physical: 0,
        count: 0,
    };
}

pub(crate) struct ChainCache {
    runs: [Run; RUNS],
    next: usize,
}

impl ChainCache {
    pub(crate) const fn new() -> Self {
        Self {
            runs: [Run::EMPTY; RUNS],
            next: 0,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.runs.fill(Run::EMPTY);
        self.next = 0;
    }

    /// Return the closest known cluster at or before the requested index.
    pub(crate) fn lookup(&self, first: u32, index: u32) -> (u32, u32) {
        let mut best = (first, 0);
        for run in &self.runs {
            if run.count == 0 || run.first != first || run.logical > index {
                continue;
            }
            let within = (index - run.logical).min(run.count - 1);
            if run.logical + within >= best.1 {
                best = (run.physical + within, run.logical + within);
            }
        }
        best
    }

    pub(crate) fn record(&mut self, first: u32, logical: u32, physical: u32) {
        for run in &mut self.runs {
            if run.count == 0 || run.first != first {
                continue;
            }
            if logical >= run.logical && logical - run.logical < run.count {
                return;
            }
            if run.logical.checked_add(run.count) == Some(logical)
                && run.physical.checked_add(run.count) == Some(physical)
            {
                if let Some(count) = run.count.checked_add(1) {
                    run.count = count;
                    return;
                }
            }
        }
        self.runs[self.next] = Run {
            first,
            logical,
            physical,
            count: 1,
        };
        self.next = (self.next + 1) % RUNS;
    }
}
