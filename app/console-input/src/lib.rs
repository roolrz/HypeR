// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Preserve terminal command boundaries in the existing byte-message transport.

/// The interactive service folds CRLF even when the UART divides it across reads.
/// The kernel console itself remains byte-transparent.
#[derive(Default)]
pub struct Newlines {
    previous_cr: bool,
}
impl Newlines {
    pub fn normalize(&mut self, bytes: &mut [u8]) -> usize {
        let mut output = 0;
        for index in 0..bytes.len() {
            let byte = bytes[index];
            let omit = self.previous_cr && byte == b'\n';
            self.previous_cr = byte == b'\r';
            if !omit {
                bytes[output] = byte;
                output += 1;
            }
        }
        output
    }
}

pub struct Packets<'a>(&'a [u8]);
impl<'a> Packets<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}
impl<'a> Iterator for Packets<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let end = self
            .0
            .iter()
            .enumerate()
            .find_map(|(index, byte)| match byte {
                // EOF must be a standalone record, never consume trailing input.
                4 => Some(if index == 0 { 1 } else { index }),
                b'\r' | b'\n' => Some(index + 1),
                _ => None,
            })
            .unwrap_or(self.0.len());
        let (packet, rest) = self.0.split_at(end);
        self.0 = rest;
        Some(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::{Newlines, Packets};
    #[test]
    fn crlf_normalization_spans_transport_reads_without_losing_other_bytes() {
        let input = b"cat\r\nabc\n\x04echo\rnext\r\n";
        for split in 0..=input.len() {
            let mut state = Newlines::default();
            let mut output = Vec::new();
            for chunk in [&input[..split], &input[split..]] {
                let mut bytes = chunk.to_vec();
                let size = state.normalize(&mut bytes);
                output.extend_from_slice(&bytes[..size]);
            }
            assert_eq!(output, b"cat\rabc\n\x04echo\rnext\r");
        }
    }

    #[test]
    fn burst_commands_and_eof_never_share_a_read_record() {
        let bytes = b"cat\nhello\n\x04echo next\r\n";
        let packets: Vec<_> = Packets::new(bytes).collect();
        assert_eq!(
            packets,
            [
                b"cat\n".as_slice(),
                b"hello\n",
                b"\x04",
                b"echo next\r",
                b"\n"
            ]
        );
        assert_eq!(packets.concat(), bytes);
    }
    #[test]
    fn every_transport_chunking_preserves_all_bytes_and_command_boundaries() {
        let bytes = b"vmm list\necho next\nabc\x04\x04";
        for split in 0..=bytes.len() {
            let packets: Vec<_> = Packets::new(&bytes[..split])
                .chain(Packets::new(&bytes[split..]))
                .collect();
            assert_eq!(packets.concat(), bytes);
            for packet in packets {
                assert!(!packet.is_empty());
                assert!(!packet[..packet.len() - 1].contains(&b'\n'));
                if packet.contains(&4) {
                    assert_eq!(packet, [4]);
                }
            }
        }
    }
}
