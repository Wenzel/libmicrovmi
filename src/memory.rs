//! This module defines a physical memory implementation behaving like a File-IO

#[cfg(test)]
use crate::api::MockIntrospectable;
use crate::microvmi::Microvmi;
use std::convert::TryFrom;
use std::io::Error;
use std::io::{ErrorKind, Result};
use std::io::{Read, Seek, SeekFrom, Write};

const PAGE_SIZE: usize = 4096;

impl Read for Microvmi {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut total_bytes_read: usize = 0;
        for chunk in buf.chunks_mut(PAGE_SIZE) {
            let mut bytes_read: u64 = 0;
            let paddr = self.stream_position()?;
            self.drv
                .read_physical(paddr, chunk, &mut bytes_read)
                .map_err(|_| Error::new(ErrorKind::Other, "driver read failure"))?;
            // advance pos from bytes_read
            self.seek(SeekFrom::Current(bytes_read as i64))?;
            // add to total
            total_bytes_read += bytes_read as usize;
        }
        Ok(total_bytes_read)
    }

    /// Read the exact number of bytes required to fill buf.
    ///
    /// Read the physical memory and add padding to fill the blanks
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut bytes_read: u64 = 0;
        for chunk in buf.chunks_mut(PAGE_SIZE) {
            let paddr = self.stream_position()?;
            self.drv
                .read_physical(paddr, chunk, &mut bytes_read)
                .unwrap_or_else(|_| chunk.fill(0));
            self.seek(SeekFrom::Current(chunk.len() as i64))?;
        }
        Ok(())
    }
}

impl Write for Microvmi {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut total_bytes_written: usize = 0;
        for chunk in buf.chunks(PAGE_SIZE) {
            let paddr = self.stream_position()?;
            self.drv
                .write_physical(paddr, chunk)
                .map_err(|_| Error::new(ErrorKind::Other, "driver write failure"))?;
            self.seek(SeekFrom::Current(chunk.len() as i64))?;
            total_bytes_written += chunk.len();
        }
        Ok(total_bytes_written)
    }

    fn flush(&mut self) -> Result<()> {
        // nothing to do
        Ok(())
    }
}

impl Seek for Microvmi {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(p) => {
                self.pos = 0;
                // force cast from u64 to i64, default to i64 MAX if conversion fail
                self.seek(SeekFrom::Current(i64::try_from(p).unwrap_or(i64::MAX)))?;
            }
            SeekFrom::End(p) => {
                self.pos = self.drv.get_max_physical_addr().map_err(|_| {
                    Error::new(ErrorKind::Other, "Failed to get maximum physical address")
                })?;
                self.seek(SeekFrom::Current(p))?;
            }
            SeekFrom::Current(p) => {
                if p > 0 {
                    self.pos = self.pos.saturating_add(p.unsigned_abs());
                } else {
                    self.pos = self.pos.saturating_sub(p.unsigned_abs());
                }
                let max_addr = self.drv.get_max_physical_addr().map_err(|_| {
                    Error::new(ErrorKind::Other, "Failed to get maximum physical address")
                })?;
                if self.pos > max_addr {
                    self.pos = max_addr;
                }
            }
        };
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seek_start() -> Result<()> {
        // create microvmi with mock driver
        let mock_introspectable = MockIntrospectable::new();
        let max_addr: u64 = 1000;
        let mut microvmi = Microvmi {
            drv: Box::new(mock_introspectable),
            pos: 0,
            max_addr,
        };
        // seek 0 doesn't move position
        assert_eq!(0, microvmi.seek(SeekFrom::Start(0))?);
        // seek beyond max_addr saturates at max_addr
        assert_eq!(max_addr, microvmi.seek(SeekFrom::Start(max_addr + 1))?);
        Ok(())
    }

    #[test]
    fn test_seek_end() -> Result<()> {
        // create microvmi with mock driver
        let mock_introspectable = MockIntrospectable::new();
        let max_addr: u64 = 1000;
        let mut microvmi = Microvmi {
            drv: Box::new(mock_introspectable),
            pos: 0,
            max_addr,
        };
        // seek end should move to max_addr
        assert_eq!(max_addr, microvmi.seek(SeekFrom::End(0))?);
        // seek end beyond should saturates to max_addr
        assert_eq!(max_addr, microvmi.seek(SeekFrom::End(50))?);
        // seek end with a negative number should update the position
        assert_eq!(max_addr - 50, microvmi.seek(SeekFrom::End(-50))?);
        // seek below 0 should saturate at 0
        assert_eq!(0, microvmi.seek(SeekFrom::End(i64::MIN))?);
        Ok(())
    }

    #[test]
    fn test_seek_current() -> Result<()> {
        // create microvmi with mock driver
        let mock_introspectable = MockIntrospectable::new();
        let max_addr: u64 = 1000;
        let mut microvmi = Microvmi {
            drv: Box::new(mock_introspectable),
            pos: 0,
            max_addr,
        };
        // seek current below 0 should saturate at 0
        assert_eq!(0, microvmi.seek(SeekFrom::Current(-5))?);
        // seek current should move the cursor
        assert_eq!(50, microvmi.seek(SeekFrom::Current(50))?);
        assert_eq!(49, microvmi.seek(SeekFrom::Current(-1))?);
        assert_eq!(59, microvmi.seek(SeekFrom::Current(10))?);
        // seek current beyond max_addr should saturate at max_addr
        assert_eq!(max_addr, microvmi.seek(SeekFrom::Current(i64::MAX))?);
        Ok(())
    }
}
