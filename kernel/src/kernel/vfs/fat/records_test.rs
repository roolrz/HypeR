// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Binding cursor coverage and bounded-stack teardown in scheduled context.

use alloc::string::String;

use super::{Error, FileRecord, Node, Record, Records, node, record_charge};
use crate::kernel::accounting::{ResourceDomain, ResourceKind};
use hyper::mm::FallibleArc;

pub(crate) fn run(domain: &ResourceDomain) -> Result<(), Error> {
    let baseline = domain.usage().committed(ResourceKind::KernelMemoryBytes);
    let live = node(FileRecord::try_new(1, domain)?, domain)?;
    let expired = node(FileRecord::try_new(2, domain)?, domain)?;
    let mut records = Records::new();
    records.insert(binding(&expired, 2, domain)?);
    // Begin with B in the examined list, then insert A in the unexamined list.
    // A prefix-first cursor must not miss B when it expires between batches.
    if !matches!(records.inspect_next(), Some(None)) {
        return Err(Error::InvalidBackendResult);
    }
    records.insert(binding(&live, 1, domain)?);
    drop(expired);
    for _ in 0..records.cleanup_bound() {
        drop(records.inspect_next());
    }
    if records.len() != 1 || records.iter().any(|record| record.id != 1) {
        return Err(Error::InvalidBackendResult);
    }
    drop(records);
    let live_usage = domain.usage().committed(ResourceKind::KernelMemoryBytes);

    // An error after a large prepared prefix exercises unwinding both owning
    // chains. Recursive Box-tail destruction would exhaust a kernel stack.
    let failed = prepare_prefix_then_fail(&live, domain);
    if !matches!(failed, Err(Error::InvalidInput))
        || domain.usage().committed(ResourceKind::KernelMemoryBytes) != live_usage
    {
        return Err(Error::InvalidBackendResult);
    }
    drop(live);
    if domain.usage().committed(ResourceKind::KernelMemoryBytes) != baseline {
        return Err(Error::InvalidBackendResult);
    }
    Ok(())
}

fn prepare_prefix_then_fail(
    owner: &FallibleArc<Node>,
    domain: &ResourceDomain,
) -> Result<Records, Error> {
    let mut records = Records::new();
    for id in 0..1024 {
        records.insert(binding(owner, id, domain)?);
    }
    for _ in 0..512 {
        drop(records.inspect_next());
    }
    Err(Error::InvalidInput)
}

fn binding(
    owner: &FallibleArc<Node>,
    id: u64,
    domain: &ResourceDomain,
) -> Result<super::PreparedRecord, Error> {
    Records::prepare(Record {
        id,
        path: String::new(),
        node: owner.downgrade(),
        content: owner.record.downgrade(),
        _charge: record_charge(domain)?,
    })
}
