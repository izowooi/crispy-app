use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

/// ZIP parsers revisit nearby central-directory fields with absolute seeks.
/// BufReader's ordinary Seek implementation discards its buffer on every such
/// call. Preserve buffered bytes and track the logical offset without syscalls.
pub(crate) struct BufferedFile {
    reader: BufReader<File>,
    position: u64,
}

impl BufferedFile {
    pub(crate) fn new(file: File) -> Self {
        Self {
            reader: BufReader::with_capacity(64 * 1024, file),
            position: 0,
        }
    }
}

impl Read for BufferedFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.reader.read(buffer)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for BufferedFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let target = match position {
            SeekFrom::Start(target) => target,
            SeekFrom::Current(offset) => {
                self.position.checked_add_signed(offset).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Seek before start of file")
                })?
            }
            SeekFrom::End(offset) => {
                self.position = self.reader.seek(SeekFrom::End(offset))?;
                return Ok(self.position);
            }
        };
        if let Ok(offset) = i64::try_from(target as i128 - self.position as i128) {
            self.reader.seek_relative(offset)?;
        } else {
            self.reader.seek(SeekFrom::Start(target))?;
        }
        self.position = target;
        Ok(target)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}
