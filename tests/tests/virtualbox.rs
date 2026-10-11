//! VirtualBox tests checking the driver against ground truth from `VBoxManage`, for any access method.
use std::convert::TryInto;
use std::fs::{self, File};
use std::ops::Range;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use log::debug;
use microvmi::api::{Introspectable, PAGE_SIZE};

use super::IntegrationTest;
use crate::common::virtualbox::{core_path, vboxmanage, vm_info, vm_name};

const PAGE: u64 = PAGE_SIZE as u64;
const CHUNK: u64 = 1 << 20;
const FOUR_GIB: u64 = 4 << 30;

/// `dumpvmcore` ELF core, deleted on drop.
struct Core {
    path: PathBuf,
    file: File,
    /// PT_LOAD segments: (guest-physical address, file offset, size)
    loads: Vec<(u64, u64, u64)>,
}

impl Core {
    fn dump() -> Core {
        // absolute: the VM process writes the file
        let path = core_path();
        let _ = fs::remove_file(&path);
        vboxmanage(&[
            "debugvm",
            &vm_name(),
            "dumpvmcore",
            &format!("--filename={}", path.display()),
        ]);
        let file = File::open(&path).expect("Failed to open core file");
        let mut core = Core {
            path,
            file,
            loads: Vec::new(),
        };
        let ehdr = core.read(0, 64);
        assert_eq!(&ehdr[..5], b"\x7fELF\x02", "not an ELF64 core");
        let u16_at = |b: &[u8], o: usize| u16::from_le_bytes(b[o..o + 2].try_into().unwrap());
        let u64_at = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let (phoff, phentsize, phnum) = (
            u64_at(&ehdr, 0x20),
            u16_at(&ehdr, 0x36),
            u16_at(&ehdr, 0x38),
        );
        assert_ne!(
            phnum, 0xffff,
            "PN_XNUM program header count is not supported"
        );
        let phdrs = core.read(phoff, phentsize as usize * phnum as usize);
        for phdr in phdrs.chunks(phentsize as usize) {
            // PT_LOAD: p_paddr is the guest-physical address
            if u32::from_le_bytes(phdr[..4].try_into().unwrap()) == 1 {
                let load = (u64_at(phdr, 0x18), u64_at(phdr, 0x08), u64_at(phdr, 0x20));
                debug!(
                    "core PT_LOAD paddr {:#x} offset {:#x} size {:#x}",
                    load.0, load.1, load.2
                );
                core.loads.push(load);
            }
        }
        core
    }

    fn read(&self, offset: u64, len: usize) -> Vec<u8> {
        let mut buf = vec![0; len];
        self.file
            .read_exact_at(&mut buf, offset)
            .expect("Failed to read core file");
        buf
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The guest's RAM ranges from VirtualBox's PGM (`info phys verbose`), independently of the driver:
/// the indented `{first:016x}-{last:016x} {type}` page runs of type `RAM`.
fn ram_ranges() -> Vec<Range<u64>> {
    let info = vboxmanage(&["debugvm", &vm_name(), "info", "phys", "verbose"]);
    let ranges: Vec<Range<u64>> = info
        .lines()
        .filter(|l| l.starts_with(' '))
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let (first, last) = words.next()?.split_once('-')?;
            (words.next()? == "RAM").then(|| {
                let hex = |s| u64::from_str_radix(s, 16).expect("bad address in info phys");
                hex(first)..hex(last) + 1
            })
        })
        .collect();
    assert!(!ranges.is_empty(), "no RAM range in info phys:\n{}", info);
    ranges
}

/// With the VM paused, every page of every RAM range reads through the driver, byte-identical to
/// the core. Non-RAM ranges in the core (ROM, VRAM, MMIO2) are not compared.
fn read_physical_matches_core(drv: Box<dyn Introspectable>) {
    // pause through VBoxManage, not the driver, so the core and the reads see the same RAM
    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    let ranges = ram_ranges();
    let core = Core::dump();
    let max_addr = drv.get_max_physical_addr().unwrap();
    assert_eq!(max_addr, ranges.iter().map(|r| r.end).max().unwrap());
    let ram_above_4g: u64 = ranges
        .iter()
        .map(|r| r.end.max(FOUR_GIB) - r.start.max(FOUR_GIB))
        .sum();
    if vm_info("memory").parse::<u64>().unwrap() > 4096 {
        assert!(max_addr > FOUR_GIB, "max physical address {:#x}", max_addr);
        assert!(ram_above_4g > 0, "no RAM range above 4 GiB");
    }

    let (mut matched, mut matched_above_4g) = (0u64, 0u64);
    let mut unreadable = Vec::new();
    for range in &ranges {
        let mut addr = range.start;
        while addr < range.end {
            let &(paddr, offset, size) = core
                .loads
                .iter()
                .find(|(p, _, s)| (*p..p + s).contains(&addr))
                .unwrap_or_else(|| panic!("RAM page {:#x} is not in the core", addr));
            // RAM ranges don't straddle 4 GiB (MMIO hole), nor do these chunks
            let len = CHUNK.min(range.end.min(paddr + size) - addr);
            let expected = core.read(offset + addr - paddr, len as usize);
            let mut buf = vec![0; len as usize];
            let mut bytes_read = 0;
            let read = drv.read_physical(addr, &mut buf, &mut bytes_read);
            // on error, compare the readable prefix and resume after the unreadable page
            let n = if read.is_ok() { len } else { bytes_read };
            assert!(
                buf[..n as usize] == expected[..n as usize],
                "RAM at {:#x}..{:#x} differs from the core",
                addr,
                addr + n
            );
            matched += n;
            if addr >= FOUR_GIB {
                matched_above_4g += n;
            }
            if read.is_err() {
                unreadable.push(addr + n);
                addr += n + PAGE;
            } else {
                addr += len;
            }
        }
    }
    println!(
        "compared {} MiB ({} MiB above 4 GiB), max physical address {:#x}",
        matched >> 20,
        matched_above_4g >> 20,
        max_addr
    );
    assert!(
        unreadable.is_empty(),
        "{} unreadable RAM pages, first {:#x?}",
        unreadable.len(),
        &unreadable[..unreadable.len().min(8)]
    );
    assert_eq!(matched, ranges.iter().map(|r| r.end - r.start).sum::<u64>());
    assert_eq!(matched_above_4g, ram_above_4g);
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_read_physical_matches_core",
    test_fn: read_physical_matches_core
});
