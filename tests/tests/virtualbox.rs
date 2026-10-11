//! VirtualBox tests checking the driver against ground truth from `VBoxManage`, for any access method.
use std::convert::TryInto;
use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use log::debug;
use microvmi::api::{Introspectable, PAGE_SIZE};

use super::IntegrationTest;
use crate::common::virtualbox::{vboxmanage, vm_info, vm_name};

const PAGE: u64 = PAGE_SIZE as u64;
const CHUNK: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

/// `dumpvmcore` ELF core, deleted on drop.
struct Core {
    path: PathBuf,
    file: File,
    /// PT_LOAD segments: (guest-physical address, file offset, size)
    loads: Vec<(u64, u64, u64)>,
}

impl Core {
    fn dump() -> Core {
        let path = std::env::temp_dir().join(format!("microvmi-test-{}.core", std::process::id()));
        // the VM process writes the file, which needs an absolute path
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
                debug!(
                    "core PT_LOAD paddr {:#x} offset {:#x} size {:#x}",
                    u64_at(phdr, 0x18),
                    u64_at(phdr, 0x08),
                    u64_at(phdr, 0x20)
                );
                core.loads
                    .push((u64_at(phdr, 0x18), u64_at(phdr, 0x08), u64_at(phdr, 0x20)));
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

/// Every guest page in the core below the driver's max physical address reads the same through
/// the driver. The core also holds non-RAM ranges (e.g. VRAM, MMIO2) that the driver may refuse:
/// pages it reports unreadable are skipped, but at least the VM's configured RAM must match.
fn read_physical_matches_core(drv: Box<dyn Introspectable>) {
    // pause through VBoxManage, not the driver, so the core and the reads see the same RAM
    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    let core = Core::dump();
    let max_addr = drv.get_max_physical_addr().unwrap();
    let ram_size = vm_info("memory").parse::<u64>().unwrap() << 20;
    if ram_size > 4 * GIB {
        assert!(
            max_addr > 4 * GIB,
            "max physical address {:#x} below 4 GiB",
            max_addr
        );
    }

    let (mut matched, mut skipped) = (0u64, 0u64);
    let mut compare = |paddr: u64, buf: &[u8], expected: &[u8]| {
        assert!(buf == expected, "page {:#x} differs from the core", paddr);
        matched += buf.len() as u64;
    };
    for &(paddr, offset, size) in &core.loads {
        let end = (paddr + size).min(max_addr);
        let mut addr = paddr;
        while addr < end {
            let len = CHUNK.min(end - addr);
            let expected = core.read(offset + addr - paddr, len as usize);
            let mut buf = vec![0; len as usize];
            let mut bytes_read = 0;
            if drv.read_physical(addr, &mut buf, &mut bytes_read).is_ok() {
                compare(addr, &buf, &expected);
            } else {
                // page by page, to skip only the unreadable pages
                for (page, expected) in (addr..)
                    .step_by(PAGE as usize)
                    .zip(expected.chunks(PAGE as usize))
                {
                    let mut buf = vec![0; expected.len()];
                    match drv.read_physical(page, &mut buf, &mut bytes_read) {
                        Ok(()) => compare(page, &buf, expected),
                        Err(_) => skipped += expected.len() as u64,
                    }
                }
            }
            addr += len;
        }
    }
    println!(
        "compared {} MiB, skipped {} KiB unreadable, max physical address {:#x}",
        matched >> 20,
        skipped >> 10,
        max_addr
    );
    // shortcut: 1 MiB slack for the legacy VGA/ROM area below 1 MiB, absent from the core's RAM
    assert!(
        matched + (1 << 20) >= ram_size,
        "only {:#x} bytes matched of {:#x} RAM",
        matched,
        ram_size
    );
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_read_physical_matches_core",
    test_fn: read_physical_matches_core
});
