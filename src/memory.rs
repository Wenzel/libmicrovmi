//! This module defines a physical memory implementation behaving like a File-IO

#[cfg(test)]
use crate::api::MockIntrospectable;
use crate::api::{Introspectable, PageFrame};
use std::cell::RefCell;
use std::convert::TryFrom;
use std::error::Error as StdError;
use std::io::Error;
use std::io::{ErrorKind, Result};
use std::io::{Read, Seek, SeekFrom, Write};
use std::rc::Rc;
use std::result::Result as StdResult;

const PAGE_SIZE: usize = 4096;

// define shared Seek behavior between the 2 Memory objects
struct AddressSeek {
    pos: u64,
    max_addr: u64,
}

impl Seek for AddressSeek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(p) => {
                self.pos = 0;
                // force cast from u64 to i64, default to i64 MAX if conversion fail
                self.seek(SeekFrom::Current(i64::try_from(p).unwrap_or(i64::MAX)))?;
            }
            SeekFrom::End(p) => {
                self.pos = self.max_addr;
                self.seek(SeekFrom::Current(p))?;
            }
            SeekFrom::Current(p) => {
                if p > 0 {
                    self.pos = self.pos.saturating_add(p.unsigned_abs());
                } else {
                    self.pos = self.pos.saturating_sub(p.unsigned_abs());
                }
                if self.pos > self.max_addr {
                    self.pos = self.max_addr;
                }
            }
        };
        Ok(self.pos)
    }
}

pub struct Memory {
    drv: Rc<RefCell<Box<dyn Introspectable>>>,
    addr_seek: AddressSeek,
}

impl Memory {
    pub fn new(drv: Rc<RefCell<Box<dyn Introspectable>>>) -> StdResult<Self, Box<dyn StdError>> {
        Ok(Memory {
            drv: drv.clone(),
            addr_seek: AddressSeek {
                pos: 0,
                max_addr: drv.borrow().get_max_physical_addr()?,
            },
        })
    }
}

impl Read for Memory {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // amount of bytes we need to read
        let mut read_remain: usize = buf.len();
        let mut bytes_read: usize = 0;
        while read_remain > 0 {
            // determine size of next chunk
            let paddr = self.stream_position()?;
            let frame = PageFrame::with_paddr(paddr);
            // windows_len -> 4K or less, if offset in frame
            let next_chunk_size = std::cmp::min(frame.window_len(), read_remain as u32) as usize;
            let chunk_end = bytes_read + next_chunk_size;
            // get chunk
            let chunk = &mut buf[bytes_read..chunk_end];
            // read the frame
            self.drv.borrow().read_frame(frame, chunk)?;
            // advance pos
            self.seek(SeekFrom::Current(next_chunk_size as i64))?;
            // update loop vars
            bytes_read += next_chunk_size;
            read_remain -= next_chunk_size;
        }
        Ok(bytes_read as usize)
    }
}

impl Write for Memory {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut total_bytes_written: usize = 0;
        for chunk in buf.chunks(PAGE_SIZE) {
            let paddr = self.stream_position()?;
            self.drv
                .borrow()
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

impl Seek for Memory {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.addr_seek.seek(pos)
    }
}

pub struct PaddedMemory {
    drv: Rc<RefCell<Box<dyn Introspectable>>>,
    addr_seek: AddressSeek,
}

impl PaddedMemory {
    pub fn new(drv: Rc<RefCell<Box<dyn Introspectable>>>) -> StdResult<Self, Box<dyn StdError>> {
        Ok(PaddedMemory {
            drv: drv.clone(),
            addr_seek: AddressSeek {
                pos: 0,
                max_addr: drv.borrow().get_max_physical_addr()?,
            },
        })
    }
}

// TODO: slight duplication
impl Read for PaddedMemory {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // amount of bytes we need to read
        let mut read_remain: usize = buf.len();
        let mut bytes_read: usize = 0;
        while read_remain > 0 {
            // determine size of next chunk
            let paddr = self.stream_position()?;
            let frame = PageFrame::with_paddr(paddr);
            // windows_len -> 4K or less, if offset in frame
            let next_chunk_size = std::cmp::min(frame.window_len(), read_remain as u32) as usize;
            let chunk_end = bytes_read + next_chunk_size;
            // get chunk
            let chunk = &mut buf[bytes_read..chunk_end];
            // read the frame
            match self.drv.borrow().read_frame(frame, chunk) {
                // handle non existing frames by padding
                Err(ref e) if e.kind() == ErrorKind::NotFound => {
                    trace!("PaddedMemory: frame not found: {:X}", frame.number);
                    chunk.fill(0)
                }
                Err(e) => return Err(e),
                _ => (),
            };
            // advance pos
            self.seek(SeekFrom::Current(next_chunk_size as i64))?;
            // update loop vars
            bytes_read += next_chunk_size;
            read_remain -= next_chunk_size;
        }
        Ok(bytes_read as usize)
    }
}

impl Seek for PaddedMemory {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.addr_seek.seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Seek
    #[test]
    fn test_seek_start() -> Result<()> {
        let max_addr: u64 = 1000;
        let mut mock_introspectable = MockIntrospectable::new();
        mock_introspectable
            .expect_get_max_physical_addr()
            .returning(move || Ok(max_addr));
        let mut memory = Memory::new(Rc::new(RefCell::new(Box::new(mock_introspectable)))).unwrap();
        // seek 0 doesn't move position
        assert_eq!(0, memory.seek(SeekFrom::Start(0))?);
        // seek beyond max_addr saturates at max_addr
        assert_eq!(max_addr, memory.seek(SeekFrom::Start(max_addr + 1))?);
        Ok(())
    }

    #[test]
    fn test_seek_end() -> Result<()> {
        let max_addr: u64 = 1000;
        let mut mock_introspectable = MockIntrospectable::new();
        mock_introspectable
            .expect_get_max_physical_addr()
            .returning(move || Ok(max_addr));
        let mut memory = Memory::new(Rc::new(RefCell::new(Box::new(mock_introspectable)))).unwrap();
        // seek end should move to max_addr
        assert_eq!(max_addr, memory.seek(SeekFrom::End(0))?);
        // seek end beyond should saturates to max_addr
        assert_eq!(max_addr, memory.seek(SeekFrom::End(50))?);
        // seek end with a negative number should update the position
        assert_eq!(max_addr - 50, memory.seek(SeekFrom::End(-50))?);
        // seek below 0 should saturate at 0
        assert_eq!(0, memory.seek(SeekFrom::End(i64::MIN))?);
        Ok(())
    }

    #[test]
    fn test_seek_current() -> Result<()> {
        let max_addr: u64 = 1000;
        let mut mock_introspectable = MockIntrospectable::new();
        mock_introspectable
            .expect_get_max_physical_addr()
            .returning(move || Ok(max_addr));
        let mut memory = Memory::new(Rc::new(RefCell::new(Box::new(mock_introspectable)))).unwrap();
        // seek current below 0 should saturate at 0
        assert_eq!(0, memory.seek(SeekFrom::Current(-5))?);
        // seek current should move the cursor
        assert_eq!(50, memory.seek(SeekFrom::Current(50))?);
        assert_eq!(49, memory.seek(SeekFrom::Current(-1))?);
        assert_eq!(59, memory.seek(SeekFrom::Current(10))?);
        // seek current beyond max_addr should saturate at max_addr
        assert_eq!(max_addr, memory.seek(SeekFrom::Current(i64::MAX))?);
        Ok(())
    }
}
