//! This module defines a physical memory implementation behaving like a File-IO

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
                self.pos = self.pos.saturating_add(p);
                if self.pos > self.max_addr {
                    self.pos = self.max_addr;
                }
            }
            SeekFrom::End(p) => {
                if p > 0 {
                    // seeking beyond the end of physical address space is not allowed
                    self.pos = self.max_addr;
                } else {
                    self.pos = self.pos.saturating_sub(u64::try_from(-p).unwrap());
                }
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
