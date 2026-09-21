// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded image read-ahead. Exactly two buffers circulate between one reader
//! and its consumer; closing either channel cancels further work, and joining
//! the scoped reader retires every buffer before the source can disappear.

use std::io;
use std::sync::mpsc;
use std::time::Duration;

pub const BATCH_BYTES: usize = 512 * 1024;

#[derive(Debug)]
pub enum Error<E> {
    Read(io::Error),
    Write(E),
    Thread(io::Error),
    WorkerStopped,
    Allocation,
    InvalidRange,
}

#[derive(Default)]
pub struct Statistics {
    pub read: Duration,
    pub write: Duration,
}

struct Batch {
    offset: u64,
    length: usize,
    bytes: Vec<u8>,
}

fn buffer(length: usize) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| ())?;
    bytes.resize(length, 0);
    Ok(bytes)
}

/// `write` receives offsets relative to the payload, never absolute file offsets.
/// A consumer failure cancels read-ahead and waits for an already-started read;
/// cancellation does not authorize reuse of a kernel-owned I/O buffer.
pub fn copy<R, W, E>(
    source_offset: u64,
    length: u64,
    read: R,
    mut write: W,
) -> Result<Statistics, Error<E>>
where
    R: Fn(u64, &mut [u8]) -> io::Result<()> + Send,
    W: FnMut(u64, &[u8]) -> Result<(), E>,
{
    source_offset
        .checked_add(length)
        .ok_or(Error::InvalidRange)?;
    if length == 0 {
        return Ok(Statistics::default());
    }
    let mut first =
        buffer(length.min(BATCH_BYTES as u64) as usize).map_err(|_| Error::Allocation)?;
    if length <= BATCH_BYTES as u64 {
        let start = tick();
        read(source_offset, &mut first).map_err(Error::Read)?;
        let read = elapsed(start);
        let start = tick();
        write(0, &first).map_err(Error::Write)?;
        return Ok(Statistics {
            read,
            write: elapsed(start),
        });
    }
    let second = buffer(BATCH_BYTES).map_err(|_| Error::Allocation)?;
    let (free_tx, free_rx) = mpsc::sync_channel(2);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    free_tx.send(first).map_err(|_| Error::WorkerStopped)?;
    free_tx.send(second).map_err(|_| Error::WorkerStopped)?;
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("image-reader".into())
            .stack_size(128 * 1024)
            .spawn_scoped(scope, move || {
                let mut total = Duration::ZERO;
                let mut completed = 0;
                while completed < length {
                    let Ok(mut bytes) = free_rx.recv() else { break };
                    let offset = source_offset + completed;
                    // After the first batch, file requests start at a batch
                    // boundary. This avoids two extra sector reads on every
                    // unaligned FIT payload transfer.
                    let capacity = BATCH_BYTES - (offset % BATCH_BYTES as u64) as usize;
                    let count = (length - completed).min(capacity as u64) as usize;
                    let start = tick();
                    let result = read(offset, &mut bytes[..count]);
                    total += elapsed(start);
                    if let Err(error) = result {
                        let _ = ready_tx.send(Err(error));
                        break;
                    }
                    if ready_tx
                        .send(Ok(Batch {
                            offset: completed,
                            length: count,
                            bytes,
                        }))
                        .is_err()
                    {
                        break;
                    }
                    completed += count as u64;
                }
                total
            })
            .map_err(Error::Thread)?;
        let mut written = Duration::ZERO;
        let result = (|| {
            let mut completed = 0;
            while completed < length {
                let batch = ready_rx
                    .recv()
                    .map_err(|_| Error::WorkerStopped)?
                    .map_err(Error::Read)?;
                if batch.offset != completed
                    || batch.length == 0
                    || batch.length as u64 > length - completed
                {
                    return Err(Error::InvalidRange);
                }
                let start = tick();
                write(completed, &batch.bytes[..batch.length]).map_err(Error::Write)?;
                written += elapsed(start);
                completed += batch.length as u64;
                // A finished reader may already have dropped its receiver.
                // The final ready batch or its error still belongs to ready_rx.
                let _ = free_tx.send(batch.bytes);
            }
            Ok(())
        })();
        drop(ready_rx);
        drop(free_tx);
        let reader = worker.join();
        result?;
        let read = reader.map_err(|_| Error::WorkerStopped)?;
        Ok(Statistics {
            read,
            write: written,
        })
    })
}

#[cfg(feature = "startup-profile")]
fn tick() -> std::time::Instant {
    std::time::Instant::now()
}
#[cfg(feature = "startup-profile")]
fn elapsed(start: std::time::Instant) -> Duration {
    start.elapsed()
}
#[cfg(not(feature = "startup-profile"))]
struct Stamp;
#[cfg(not(feature = "startup-profile"))]
fn tick() -> Stamp {
    Stamp
}
#[cfg(not(feature = "startup-profile"))]
fn elapsed(_: Stamp) -> Duration {
    Duration::ZERO
}

#[cfg(test)]
#[path = "../tests/image_io.rs"]
mod tests;
