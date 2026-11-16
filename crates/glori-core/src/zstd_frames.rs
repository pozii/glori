use std::io::{self, BufRead, Read};

use ruzstd::decoding::{FrameDecoder, StreamingDecoder};

enum State<R: BufRead> {
    Decoding(Box<StreamingDecoder<R, FrameDecoder>>),
    Between(R),
    Finished,
    Failed,
}

pub struct ZstdFrames<R: BufRead> {
    state: State<R>,
}

impl<R: BufRead> ZstdFrames<R> {
    pub fn new(source: R) -> io::Result<Self> {
        Ok(ZstdFrames {
            state: Self::start(source)?,
        })
    }

    fn start(mut source: R) -> io::Result<State<R>> {
        if source.fill_buf()?.is_empty() {
            return Ok(State::Finished);
        }
        StreamingDecoder::new(source)
            .map(|decoder| State::Decoding(Box::new(decoder)))
            .map_err(|error| io::Error::other(error.to_string()))
    }
}

impl<R: BufRead> Read for ZstdFrames<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            match std::mem::replace(&mut self.state, State::Failed) {
                State::Decoding(mut decoder) => {
                    let read = decoder.read(buf)?;
                    if read > 0 {
                        self.state = State::Decoding(decoder);
                        return Ok(read);
                    }
                    self.state = State::Between((*decoder).into_inner());
                }
                State::Between(source) => self.state = Self::start(source)?,
                State::Finished => {
                    self.state = State::Finished;
                    return Ok(0);
                }
                State::Failed => {
                    return Err(io::Error::other("the zstd stream is corrupted"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn empty_input_is_an_empty_stream() {
        let mut reader = ZstdFrames::new(Cursor::new(Vec::new())).unwrap();
        let mut out = Vec::new();
        assert_eq!(reader.read_to_end(&mut out).unwrap(), 0);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(ZstdFrames::new(Cursor::new(b"definitely not zstd".to_vec())).is_err());
    }

    #[test]
    fn an_error_is_not_followed_by_a_clean_end_of_stream() {
        let mut bytes = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00];
        bytes.extend_from_slice(b"truncated");
        let Ok(mut reader) = ZstdFrames::new(Cursor::new(bytes)) else {
            return;
        };
        let mut buffer = [0u8; 64];
        let first = reader.read(&mut buffer);
        if first.is_err() {
            assert!(reader.read(&mut buffer).is_err());
        }
    }
}
