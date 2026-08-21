//! Kernel log ring-buffer record and wraparound behavior.

use hyper::log::{Level, ReadResult, RecordFlags, RingBuffer};

#[test]
fn preserves_record_metadata_across_wraparound() {
    let mut ring = RingBuffer::<64>::new();
    let first = crate::require_ok(ring.append(Level::Info, b"first", RecordFlags::NONE));
    let second = crate::require_ok(ring.append(Level::Warning, b"second", RecordFlags::NONE));
    assert_eq!(first, 0);
    assert_eq!(second, 1);

    let mut output = [0; 16];
    let record = match crate::require_ok(ring.read(second, &mut output)) {
        ReadResult::Record(record) => record,
        result => panic!("required a record, received {result:?}"),
    };
    assert_eq!(record.level, Level::Warning);
    assert_eq!(&output[..record.copied], b"second");

    for index in 0..8u8 {
        crate::require_ok(ring.append(Level::Debug, &[index; 12], RecordFlags::NONE));
    }
    assert!(ring.dropped() != 0);
    assert!(matches!(
        crate::require_ok(ring.read(first, &mut output)),
        ReadResult::Overrun { .. }
    ));
}

#[test]
fn truncates_a_record_that_exceeds_the_ring_capacity() {
    let mut ring = RingBuffer::<32>::new();
    let sequence = crate::require_ok(ring.append(
        Level::Error,
        b"a message that cannot fit in this tiny ring",
        RecordFlags::NONE,
    ));
    let mut output = [0; 32];
    let record = match crate::require_ok(ring.read(sequence, &mut output)) {
        ReadResult::Record(record) => record,
        result => panic!("required a record, received {result:?}"),
    };
    assert!(record.flags.contains(RecordFlags::TRUNCATED));
    assert_eq!(record.length, 16);
}

#[test]
fn reports_empty_buffers_and_partial_reads() {
    let mut ring = RingBuffer::<64>::new();
    let mut output = [0; 3];
    assert_eq!(
        crate::require_ok(ring.read(0, &mut output)),
        ReadResult::Empty { next_sequence: 0 }
    );

    let sequence = crate::require_ok(ring.append(Level::Notice, b"abcdef", RecordFlags::NONE));
    let record = match crate::require_ok(ring.read(sequence, &mut output)) {
        ReadResult::Record(record) => record,
        result => panic!("required a record, received {result:?}"),
    };
    assert_eq!(record.length, 6);
    assert_eq!(record.copied, 3);
    assert_eq!(&output, b"abc");
}

#[test]
fn rejects_a_ring_smaller_than_its_record_header() {
    let mut ring = RingBuffer::<8>::new();
    assert_eq!(
        ring.append(Level::Info, b"message", RecordFlags::NONE),
        Err(hyper::log::AppendError::BufferTooSmall)
    );
}
