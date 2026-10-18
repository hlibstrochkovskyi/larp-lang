//! Explicit, borrowed host input capabilities. No ambient process access.

use std::io::{self, BufRead};

pub const MAX_ARGUMENTS: usize = 4096;
pub const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 64 * 1024;
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Failure {
    Denied(&'static str),
    Resource(&'static str),
}
impl Failure {
    pub(crate) fn diagnostic(self) -> (&'static str, String) {
        match self {
            Self::Denied(message) => ("LARP-R010", message.into()),
            Self::Resource(message) => ("LARP-R006", message.into()),
        }
    }
}

#[derive(Clone, Copy, Default)]
enum InputState {
    #[default]
    Open,
    End,
    Failed,
    Exhausted,
}

/// Each optional value grants exactly one input capability. Default denies both.
/// The input cursor and byte budget persist across executions using this host.
#[derive(Default)]
pub struct Host<'a> {
    arguments: Option<&'a [String]>,
    input: Option<&'a mut dyn BufRead>,
    consumed: usize,
    state: InputState,
}
impl<'a> Host<'a> {
    pub fn new(arguments: Option<&'a [String]>, input: Option<&'a mut dyn BufRead>) -> Self {
        Self {
            arguments,
            input,
            ..Self::default()
        }
    }

    pub(crate) fn arguments(&self) -> Result<&[String], Failure> {
        let arguments = self
            .arguments
            .ok_or(Failure::Denied("Argument capability was not granted"))?;
        if arguments.len() > MAX_ARGUMENTS {
            return Err(Failure::Resource("Argument count or byte limit exceeded"));
        }
        let bytes = arguments
            .iter()
            .try_fold(0usize, |total, argument| total.checked_add(argument.len()));
        if bytes.is_none_or(|bytes| bytes > MAX_ARGUMENT_BYTES) {
            return Err(Failure::Resource("Argument count or byte limit exceeded"));
        }
        Ok(arguments)
    }

    pub(crate) fn read_line(&mut self) -> Result<Result<Option<String>, &'static str>, Failure> {
        let reader = self
            .input
            .as_mut()
            .ok_or(Failure::Denied("Standard-input capability was not granted"))?;
        match self.state {
            InputState::End => return Ok(Ok(None)),
            InputState::Failed => return Ok(Err("input read failed")),
            InputState::Exhausted => {
                return Err(Failure::Resource("Input line or total byte limit exceeded"));
            }
            InputState::Open => {}
        }
        let mut bytes = Vec::new();
        loop {
            let buffer = match reader.fill_buf() {
                Ok(buffer) => buffer,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    // A partial record cannot be resumed as though it were whole.
                    self.state = InputState::Failed;
                    return Ok(Err("input read failed"));
                }
            };
            if buffer.is_empty() {
                self.state = InputState::End;
                if bytes.is_empty() {
                    return Ok(Ok(None));
                }
                break;
            }
            let available = MAX_LINE_BYTES
                .saturating_sub(bytes.len())
                .min(MAX_INPUT_BYTES.saturating_sub(self.consumed));
            if available == 0 {
                self.state = InputState::Exhausted;
                return Err(Failure::Resource("Input line or total byte limit exceeded"));
            }
            let end = buffer
                .iter()
                .take(available + 1)
                .position(|byte| *byte == b'\n');
            let wanted = end.map_or(buffer.len(), |position| position + 1);
            let count = wanted.min(available);
            if bytes.try_reserve(count).is_err() {
                self.state = InputState::Exhausted;
                return Err(Failure::Resource("Cannot allocate input line"));
            }
            bytes.extend_from_slice(&buffer[..count]);
            reader.consume(count);
            self.consumed += count;
            if end.is_some() && count == wanted {
                break;
            }
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        Ok(String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "input is not valid UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor, Read};

    #[test]
    fn denied_capabilities_differ_from_empty_arguments_and_eof() {
        assert!(matches!(
            Host::default().arguments(),
            Err(Failure::Denied(_))
        ));
        assert!(matches!(
            Host::default().read_line(),
            Err(Failure::Denied(_))
        ));
        let mut input = Cursor::new(b"");
        let mut host = Host::new(Some(&[]), Some(&mut input));
        assert!(host.arguments().unwrap().is_empty());
        assert_eq!(host.read_line().unwrap(), Ok(None));
        assert_eq!(host.read_line().unwrap(), Ok(None));
    }

    #[test]
    fn records_distinguish_blank_eof_crlf_final_cr_and_invalid_utf8() {
        let mut input =
            BufReader::with_capacity(1, Cursor::new(b"\n\r\nhi\0\n\xff\nnext\nfinal\r"));
        let mut host = Host::new(None, Some(&mut input));
        for expected in [
            Ok(Some("".into())),
            Ok(Some("".into())),
            Ok(Some("hi\0".into())),
            Err("input is not valid UTF-8"),
            Ok(Some("next".into())),
            Ok(Some("final\r".into())),
            Ok(None),
        ] {
            assert_eq!(host.read_line().unwrap(), expected);
        }
    }

    #[test]
    fn physical_line_and_total_limits_include_terminators_and_accept_exact_eof() {
        let mut exact = Cursor::new(vec![b'x'; MAX_LINE_BYTES]);
        let mut host = Host::new(None, Some(&mut exact));
        assert_eq!(
            host.read_line().unwrap().unwrap().unwrap().len(),
            MAX_LINE_BYTES
        );
        assert_eq!(host.read_line().unwrap(), Ok(None));
        let mut excess = Cursor::new(vec![b'x'; MAX_LINE_BYTES + 1]);
        let mut host = Host::new(None, Some(&mut excess));
        assert!(matches!(host.read_line(), Err(Failure::Resource(_))));
        assert_eq!(host.consumed, MAX_LINE_BYTES);
        assert!(matches!(host.read_line(), Err(Failure::Resource(_))));
        assert_eq!(host.consumed, MAX_LINE_BYTES);
        let mut text = vec![b'x'; MAX_LINE_BYTES - 1];
        text.push(b'\n');
        let mut total = Cursor::new(text.repeat(MAX_INPUT_BYTES / MAX_LINE_BYTES + 1));
        let mut host = Host::new(None, Some(&mut total));
        for _ in 0..MAX_INPUT_BYTES / MAX_LINE_BYTES {
            host.read_line().unwrap().unwrap();
        }
        assert!(matches!(host.read_line(), Err(Failure::Resource(_))));
        assert_eq!(host.consumed, MAX_INPUT_BYTES);
    }

    #[test]
    fn utf8_scalars_can_cross_every_host_buffer_boundary() {
        let mut input = BufReader::with_capacity(1, Cursor::new("🦀\r\né\n".as_bytes()));
        let mut host = Host::new(None, Some(&mut input));
        assert_eq!(host.read_line().unwrap(), Ok(Some("🦀".into())));
        assert_eq!(host.read_line().unwrap(), Ok(Some("é".into())));
        assert_eq!(host.read_line().unwrap(), Ok(None));
    }

    #[test]
    fn argument_limits_are_checked_before_copying_values() {
        let arguments = vec![String::new(); MAX_ARGUMENTS + 1];
        assert!(matches!(
            Host::new(Some(&arguments), None).arguments(),
            Err(Failure::Resource(_))
        ));
        let arguments = vec!["x".repeat(MAX_ARGUMENT_BYTES + 1)];
        assert!(matches!(
            Host::new(Some(&arguments), None).arguments(),
            Err(Failure::Resource(_))
        ));
        let arguments = vec!["x".repeat(MAX_ARGUMENT_BYTES)];
        assert_eq!(
            Host::new(Some(&arguments), None).arguments().unwrap()[0].len(),
            MAX_ARGUMENT_BYTES
        );
    }

    struct Failing {
        calls: usize,
    }
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            unreachable!()
        }
    }
    impl BufRead for Failing {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.calls += 1;
            match self.calls {
                1 => Err(io::ErrorKind::Interrupted.into()),
                2 => Ok(b"partial"),
                3 => Err(io::ErrorKind::Other.into()),
                _ => panic!("Failed input must stay latched"),
            }
        }
        fn consume(&mut self, count: usize) {
            assert_eq!(count, 7);
        }
    }
    #[test]
    fn interrupted_reads_retry_but_partial_io_failures_are_latched() {
        let mut input = Failing { calls: 0 };
        let mut host = Host::new(None, Some(&mut input));
        assert_eq!(host.read_line().unwrap(), Err("input read failed"));
        assert_eq!(host.read_line().unwrap(), Err("input read failed"));
        assert_eq!(host.consumed, 7);
        assert_eq!(input.calls, 3);
    }
}

#[cfg(test)]
mod regional_tests {
    use super::*;
    use crate::heap::Heap;
    use larp_types::Intrinsic;
    use std::io::Read;

    struct Unreadable;
    impl Read for Unreadable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("Regional code must not read host input")
        }
    }
    impl BufRead for Unreadable {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            panic!("Regional code must not inspect host input")
        }
        fn consume(&mut self, _: usize) {
            panic!("Regional code must not consume host input")
        }
    }
    #[test]
    fn runtime_region_guard_rejects_input_before_reading_or_allocating() {
        let arguments = vec!["real".into()];
        let mut input = Unreadable;
        let mut host = Host::new(Some(&arguments), Some(&mut input));
        let mut heap = Heap::default();
        for operation in [Intrinsic::Arguments, Intrinsic::ReadLine] {
            let error = crate::host_intrinsic(&operation, &mut host, &mut heap, true).unwrap_err();
            assert_eq!(error.0, "LARP-R010");
        }
        assert_eq!(heap.allocated, 0);
    }
}
