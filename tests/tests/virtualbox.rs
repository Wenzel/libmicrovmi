//! VirtualBox tests checking the driver against ground truth from `VBoxManage`, for any access method.
use std::convert::TryInto;
use std::env;
use std::fs::{self, File};
use std::ops::Range;
use std::os::unix::fs::FileExt;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use log::debug;
use microvmi::api::registers::{Registers, X86Registers};
use microvmi::api::{Introspectable, PAGE_SIZE};

use super::IntegrationTest;
use crate::common::virtualbox::{core_path, try_vboxmanage, vboxmanage, vm_info, vm_name};

const PAGE: u64 = PAGE_SIZE as u64;
const CHUNK: u64 = 1 << 20;
const FOUR_GIB: u64 = 4 << 30;

/// `dumpvmcore` ELF core, unlinked once open: its disk space is freed on drop.
struct Core {
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
        // the next dump reuses the path
        fs::remove_file(&path).expect("Failed to unlink core file");
        let mut core = Core {
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

    /// The PT_LOAD segment (guest-physical address, file offset, size) holding guest page `addr`.
    fn load(&self, addr: u64) -> (u64, u64, u64) {
        *self
            .loads
            .iter()
            .find(|(p, _, s)| (*p..p + s).contains(&addr))
            .unwrap_or_else(|| panic!("RAM page {:#x} is not in the core", addr))
    }

    /// Guest RAM `addr..addr + len`, within one PT_LOAD segment.
    fn ram(&self, addr: u64, len: u64) -> Vec<u8> {
        let (paddr, offset, size) = self.load(addr);
        assert!(
            addr + len <= paddr + size,
            "{:#x}+{:#x} spans PT_LOADs",
            addr,
            len
        );
        self.read(offset + addr - paddr, len as usize)
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

/// Measures the end-to-end batched read path over all RAM, without core-dump I/O.
fn read_physical_benchmark(drv: Box<dyn Introspectable>) {
    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    let ranges = ram_ranges();
    let start = Instant::now();
    let mut bytes = 0u64;
    let mut buf = vec![0; CHUNK as usize];
    for range in ranges {
        let mut addr = range.start;
        while addr < range.end {
            let len = CHUNK.min(range.end - addr) as usize;
            let mut bytes_read = 0;
            drv.read_physical(addr, &mut buf[..len], &mut bytes_read)
                .unwrap_or_else(|e| {
                    panic!("read at {:#x} after {} bytes: {}", addr, bytes_read, e)
                });
            assert_eq!(bytes_read, len as u64);
            bytes += len as u64;
            addr += len as u64;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "read {} MiB in {:.3}s ({:.1} MiB/s)",
        bytes >> 20,
        elapsed.as_secs_f64(),
        bytes as f64 / (1 << 20) as f64 / elapsed.as_secs_f64()
    );
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_read_physical_benchmark",
    test_fn: read_physical_benchmark
});

inventory::submit!(IntegrationTest {
    name: "virtualbox_read_physical_matches_core",
    test_fn: read_physical_matches_core
});

/// Per-vCPU statistic `/TM/CPU/<n>/<name>` of the VMM, in vCPU order.
fn vcpu_stats(name: &str) -> Vec<u64> {
    let pattern = format!("/TM/CPU/*/{}", name);
    let xml = vboxmanage(&["debugvm", &vm_name(), "statistics", "--pattern", &pattern]);
    // <U64 val="123" unit="ns" name="/TM/CPU/00/cNsExecuting"/>
    let stats: Vec<u64> = xml
        .lines()
        .filter(|l| l.contains(&format!("/{}\"", name)))
        .map(|l| {
            let val = l.split("val=\"").nth(1).and_then(|v| v.split('"').next());
            val.and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("bad statistics line {:?}", l))
        })
        .collect();
    assert!(!stats.is_empty(), "no {} statistic:\n{}", pattern, xml);
    stats
}

/// The vCPU count matches the vCPUs the VMM really runs (one `/TM/CPU/<n>` statistics tree per
/// vCPU), independently of the VM configuration. The multi-vCPU check needs `VBOX_TEST_CPUS=2`.
fn vcpu_count_matches_vmm(drv: Box<dyn Introspectable>) {
    let count = drv.get_vcpu_count().unwrap();
    assert_eq!(usize::from(count), vcpu_stats("cNsTotal").len());
    if let Ok(cpus) = std::env::var("VBOX_TEST_CPUS") {
        assert_eq!(count.to_string(), cpus);
    }
    print!("{} vCPU(s) ", count);
    if count == 1 {
        print!("(set VBOX_TEST_CPUS=2 for the multi-vCPU check) ");
    }
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_vcpu_count_matches_vmm",
    test_fn: vcpu_count_matches_vmm
});

/// Pause stops every vCPU before it returns, independently of the state VirtualBox reports: each
/// vCPU's guest-executing time is frozen while paused, and the total grows again after resume.
fn pause_stops_vcpus(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    let paused = vcpu_stats("cNsExecuting");
    sleep(Duration::from_secs(1));
    assert_eq!(
        vcpu_stats("cNsExecuting"),
        paused,
        "a vCPU executed while paused"
    );
    drv.resume().unwrap();
    sleep(Duration::from_secs(1));
    let resumed = vcpu_stats("cNsExecuting");
    assert!(
        resumed.iter().sum::<u64>() > paused.iter().sum::<u64>(),
        "no vCPU executed after resume: {:?}",
        resumed
    );
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_pause_stops_vcpus",
    test_fn: pause_stops_vcpus
});

/// Pause is VirtualBox's own, visible to `VBoxManage` as soon as it returns, and idempotent, also
/// when another client paused or resumed the VM: one resume undoes any number of pauses.
fn pause_idempotent_and_visible(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    drv.pause().unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    drv.resume().unwrap();
    assert_eq!(vm_info("VMState"), "running");
    drv.resume().unwrap();
    assert_eq!(vm_info("VMState"), "running");

    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    drv.pause().unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    drv.resume().unwrap();
    assert_eq!(vm_info("VMState"), "running");
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_pause_idempotent_and_visible",
    test_fn: pause_idempotent_and_visible
});

/// Restores the original page contents and the running VM state, including during unwinding.
struct RestorePages<'a> {
    drv: &'a dyn Introspectable,
    pages: &'a [(u64, Vec<u8>)],
    armed: bool,
}

impl RestorePages<'_> {
    /// The test keeps the VM paused before calling this.
    fn restore(&mut self) {
        for (page, original) in self.pages.iter() {
            self.drv.write_physical(*page, original).unwrap();
        }
    }
}

impl Drop for RestorePages<'_> {
    fn drop(&mut self) {
        if self.armed {
            let vm = vm_name();
            let _ = try_vboxmanage(&["controlvm", &vm, "pause"]);
            for (page, original) in self.pages.iter() {
                let _ = self.drv.write_physical(*page, original);
            }
        }
        let vm = vm_name();
        let _ = try_vboxmanage(&["controlvm", &vm, "resume"]);
    }
}

/// Pages to write to: zero in `core`, so most likely unused by the idle guest. The first zero page
/// below each of: the top of Linux's reserved low 64 KiB (never allocated by the guest), the
/// middle of RAM, the top of RAM below 4 GiB and the top of RAM (above 4 GiB if there is RAM there).
fn zero_pages(core: &Core, ranges: &[Range<u64>]) -> Vec<u64> {
    let top = ranges.iter().map(|r| r.end).max().unwrap();
    let top_below_4g = ranges
        .iter()
        .map(|r| r.end)
        .filter(|&e| e <= FOUR_GIB)
        .max()
        .unwrap();
    let mut pages: Vec<u64> = [0x10000, top / 2, top_below_4g, top]
        .iter()
        .map(|&target| {
            let range = ranges
                .iter()
                .find(|r| r.start < target && target <= r.end)
                .unwrap_or_else(|| panic!("{:#x} is not in a RAM range", target));
            // page 0 excluded: real-mode IVT
            (1..=1024)
                .map(|i| (target & !(PAGE - 1)) - i * PAGE)
                .take_while(|&p| p >= range.start.max(PAGE))
                .find(|&p| core.ram(p, PAGE).iter().all(|&b| b == 0))
                .unwrap_or_else(|| panic!("no zero page below {:#x}", target))
        })
        .collect();
    pages.sort_unstable();
    pages.dedup();
    pages
}

/// With the VM paused, a random pattern written to RAM pages through the driver reads back and is
/// present in a fresh core, where the rest of RAM is unchanged. A second fresh core verifies the
/// patterns after a resume/re-pause cycle. The exact original bytes and running state are restored
/// on success and during unwinding. `VBOX_TEST_SEED` replays a pattern.
fn write_physical_matches_core(drv: Box<dyn Introspectable>) {
    let vm = vm_name();
    vboxmanage(&["controlvm", &vm, "pause"]);
    let ranges = ram_ranges();
    let before = Core::dump();
    let page_addrs = zero_pages(&before, &ranges);
    if ranges.iter().any(|r| r.end > FOUR_GIB) {
        assert!(
            page_addrs.iter().any(|&page| page >= FOUR_GIB),
            "write test did not select RAM above 4 GiB"
        );
    }
    let pages: Vec<(u64, Vec<u8>)> = page_addrs
        .iter()
        .map(|&page| (page, before.ram(page, PAGE)))
        .collect();
    let mut restore = RestorePages {
        drv: &*drv,
        pages: &pages,
        armed: true,
    };
    let seed = env::var("VBOX_TEST_SEED").map_or_else(
        |_| {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
            now.unwrap().as_nanos() as u64
        },
        |s| s.parse().expect("VBOX_TEST_SEED is not a u64"),
    );
    let hex: Vec<String> = page_addrs.iter().map(|p| format!("{:#x}", p)).collect();
    print!("seed {} pages {} ", seed, hex.join(" "));
    let mut state = seed | 1; // xorshift64: nonzero state
    let mut random_byte = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as u8
    };
    let patterns: Vec<Vec<u8>> = page_addrs
        .iter()
        .map(|_| (0..PAGE).map(|_| random_byte()).collect())
        .collect();

    let top = ranges.iter().map(|r| r.end).max().unwrap();
    // local APIC MMIO, the first page above RAM
    for addr in [0xfee0_0000, top] {
        assert!(
            drv.write_physical(addr, &[0xcc; PAGE as usize]).is_err(),
            "write to non-RAM {:#x} succeeded",
            addr
        );
    }

    let read_page = |page| {
        let mut buf = vec![0; PAGE as usize];
        drv.read_physical(page, &mut buf, &mut 0).unwrap();
        buf
    };
    for (&page, pattern) in page_addrs.iter().zip(&patterns) {
        drv.write_physical(page, pattern).unwrap();
        assert!(
            read_page(page) == *pattern,
            "page {:#x} reads back differently",
            page
        );
    }

    let after = Core::dump();
    for range in &ranges {
        let mut addr = range.start;
        while addr < range.end {
            let (paddr, _, size) = before.load(addr);
            let len = CHUNK.min(range.end.min(paddr + size) - addr);
            let mut expected = before.ram(addr, len);
            for (&page, pattern) in page_addrs.iter().zip(&patterns) {
                if (addr..addr + len).contains(&page) {
                    let at = (page - addr) as usize;
                    expected[at..at + PAGE as usize].copy_from_slice(pattern);
                }
            }
            let actual = after.ram(addr, len);
            if let Some(i) = (0..len as usize).find(|&i| actual[i] != expected[i]) {
                panic!(
                    "core differs from the expected RAM at {:#x}",
                    addr + i as u64
                );
            }
            addr += len;
        }
    }
    drop((before, after));

    vboxmanage(&["controlvm", &vm, "resume"]);
    vboxmanage(&["controlvm", &vm, "pause"]);
    let resumed = Core::dump();
    for (&page, pattern) in page_addrs.iter().zip(&patterns) {
        assert!(
            resumed.ram(page, PAGE) == *pattern,
            "page {:#x} changed over resume",
            page
        );
    }
    drop(resumed);

    restore.restore();
    let restored = Core::dump();
    for (page, original) in &pages {
        assert_eq!(
            restored.ram(*page, PAGE),
            *original,
            "page {:#x} not restored",
            page
        );
    }
    restore.armed = false;
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_write_physical_matches_core",
    test_fn: write_physical_matches_core
});

fn x86_registers(drv: &dyn Introspectable, vcpu: u16) -> X86Registers {
    let Registers::X86(regs) = drv.read_registers(vcpu).unwrap();
    regs
}

/// Keeps a paused VM's original registers and restores them before resuming, including while a
/// failing assertion unwinds the test.
struct RegisterRestore {
    drv: Box<dyn Introspectable>,
    vcpu: u16,
    original: X86Registers,
    active: bool,
}

impl RegisterRestore {
    fn new(mut drv: Box<dyn Introspectable>) -> Self {
        drv.pause().unwrap();
        let vcpu = drv.get_vcpu_count().unwrap() - 1;
        let original = x86_registers(drv.as_ref(), vcpu);
        RegisterRestore {
            drv,
            vcpu,
            original,
            active: true,
        }
    }

    fn finish(mut self) {
        self.drv
            .write_registers(self.vcpu, Registers::X86(self.original.clone()))
            .unwrap();
        assert_eq!(x86_registers(self.drv.as_ref(), self.vcpu), self.original);
        self.drv.resume().unwrap();
        self.active = false;
    }
}

impl Drop for RegisterRestore {
    fn drop(&mut self) {
        if self.active {
            if let Err(error) = self
                .drv
                .write_registers(self.vcpu, Registers::X86(self.original.clone()))
            {
                eprintln!("failed to restore vCPU {} registers: {}", self.vcpu, error);
            }
            if let Err(error) = self.drv.resume() {
                eprintln!("failed to resume VM after register test: {}", error);
            }
        }
    }
}

/// While paused, two reads of every vCPU's registers are identical, not all zero, and RFLAGS
/// is architecturally valid; a vCPU past the count is an error. The multi-vCPU check needs
/// `VBOX_TEST_CPUS=2`.
fn registers_unchanged_while_paused(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    let count = drv.get_vcpu_count().unwrap();
    assert!(drv.read_registers(count).is_err(), "vCPU {} read", count);
    for vcpu in 0..count {
        let first = x86_registers(drv.as_ref(), vcpu);
        sleep(Duration::from_millis(500));
        let second = x86_registers(drv.as_ref(), vcpu);
        assert_eq!(first, second);
        assert_ne!(first, X86Registers::default());
        // architectural RFLAGS: bit 1 always set, bits 63:22 reserved (zero)
        assert_eq!(first.rflags & !0x3f_ffff, 0, "rflags {:#x}", first.rflags);
        assert_ne!(first.rflags & 2, 0, "rflags {:#x}", first.rflags);
    }
    drv.resume().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_registers_unchanged_while_paused",
    test_fn: registers_unchanged_while_paused
});

/// Guest-physical address that the 4-level page tables at `cr3` map `vaddr` to, if mapped.
fn translate(drv: &dyn Introspectable, cr3: u64, vaddr: u64) -> Option<u64> {
    const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
    let mut table = cr3 & ADDR_MASK;
    // PML4, PDPT, PD, PT
    for shift in [39, 30, 21, 12] {
        let mut entry = [0; 8];
        let mut bytes_read = 0;
        drv.read_physical(
            table + ((vaddr >> shift) & 0x1ff) * 8,
            &mut entry,
            &mut bytes_read,
        )
        .ok()?;
        let entry = u64::from_le_bytes(entry);
        if entry & 1 == 0 {
            return None;
        }
        // 1 GiB and 2 MiB pages
        let large = shift != 39 && shift != 12 && entry & 0x80 != 0;
        if large || shift == 12 {
            let offset_mask = (1 << shift) - 1;
            return Some((entry & ADDR_MASK & !offset_mask) | (vaddr & offset_mask));
        }
        table = entry & ADDR_MASK;
    }
    unreachable!()
}

/// The registers make sense against guest memory: walking each vCPU's page tables from CR3
/// translates RIP and the IDT base to readable guest-physical RAM, and the Linux banner is in RAM.
fn registers_match_guest_memory(mut drv: Box<dyn Introspectable>) {
    // a cold-booted guest (VBOX_TEST_CPUS, VBOX_TEST_MEMORY_MB) may still be booting
    let long_mode = |drv: &dyn Introspectable| {
        (0..drv.get_vcpu_count().unwrap())
            .all(|vcpu| x86_registers(drv, vcpu).efer & (1 << 10) != 0)
    };
    for _ in 0..30 {
        if long_mode(drv.as_ref()) {
            break;
        }
        sleep(Duration::from_millis(500));
    }
    drv.pause().unwrap();
    let max_addr = drv.get_max_physical_addr().unwrap();
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        let regs = x86_registers(drv.as_ref(), vcpu);
        assert_ne!(regs.efer & (1 << 10), 0, "vCPU {} not in long mode", vcpu);
        assert_eq!(regs.cr4 & (1 << 12), 0, "vCPU {}: 5-level paging", vcpu);
        for (what, vaddr) in [("RIP", regs.rip), ("IDT base", regs.idt.base)] {
            let paddr = translate(drv.as_ref(), regs.cr3, vaddr)
                .unwrap_or_else(|| panic!("vCPU {}: {} {:#x} is not mapped", vcpu, what, vaddr));
            assert!(paddr < max_addr, "{} maps to {:#x}", what, paddr);
            let mut page = vec![0; PAGE as usize];
            let mut bytes_read = 0;
            drv.read_physical(paddr & !(PAGE - 1), &mut page, &mut bytes_read)
                .unwrap_or_else(|e| panic!("{} page at {:#x}: {}", what, paddr, e));
            assert!(page.iter().any(|&b| b != 0), "{} page is all zeros", what);
            print!("vCPU {} {} {:#x} -> {:#x}, ", vcpu, what, vaddr, paddr);
        }
    }

    let banner = b"Linux version ";
    let mut buf = vec![0; CHUNK as usize];
    let found = (0..max_addr).step_by(CHUNK as usize).find_map(|addr| {
        let len = CHUNK.min(max_addr - addr) as usize;
        let mut bytes_read = 0;
        // MMIO holes are unreadable: search the readable prefix
        let _ = drv.read_physical(addr, &mut buf[..len], &mut bytes_read);
        // shortcut: a banner straddling two chunks is missed, fine for a fixed test guest
        buf[..bytes_read as usize]
            .windows(banner.len())
            .position(|w| w == banner)
            .map(|i| addr + i as u64)
    });
    let found = found.expect("no Linux banner in guest RAM");
    let mut line = vec![0; 80];
    let mut bytes_read = 0;
    drv.read_physical(found, &mut line, &mut bytes_read)
        .unwrap();
    let line = String::from_utf8_lossy(&line);
    print!("{:?} at {:#x} ", line.split('\0').next().unwrap(), found);
    drv.resume().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_registers_match_guest_memory",
    test_fn: registers_match_guest_memory
});

/// While paused, representative registers from every writable group are changed and read back,
/// including through `VBoxManage`; the guard restores the complete original set before resuming
/// and also restores it if an assertion fails.
fn write_registers_reads_back(drv: Box<dyn Introspectable>) {
    let restore = RegisterRestore::new(drv);
    let vcpu = restore.vcpu;
    let mut expected = restore.original.clone();

    let cs_dpl = vboxmanage(&[
        "debugvm",
        &vm_name(),
        "getregisters",
        "--cpu",
        &vcpu.to_string(),
        "cs_attr.dpl",
    ]);
    let dpl = cs_dpl
        .split_once('=')
        .and_then(|(_, value)| {
            let value = value.trim();
            value.strip_prefix("0x").map_or_else(
                || value.parse().ok(),
                |hex| u8::from_str_radix(hex, 16).ok(),
            )
        })
        .unwrap_or_else(|| panic!("unexpected cs_attr.dpl output: {:?}", cs_dpl));
    assert!(dpl <= 3, "invalid CS DPL {}", dpl);

    let mut invalid = restore.original.clone();
    invalid.rflags = 0; // bit 1 clear
    let rflags_error = restore
        .drv
        .write_registers(vcpu, Registers::X86(invalid))
        .expect_err("an RFLAGS value with bit 1 clear was accepted");
    print!("invalid RFLAGS refused: {}, ", rflags_error);
    assert_eq!(
        x86_registers(restore.drv.as_ref(), vcpu),
        restore.original,
        "invalid RFLAGS write changed the registers"
    );

    expected.r12 ^= 0x5a5a_5a5a_5a5a_5a5a;
    expected.rip ^= 1;
    expected.rflags ^= 1; // CF
    expected.cr0 ^= 1 << 3; // TS
    expected.cr2 ^= 1 << 12;
    expected.cr3 ^= 1 << 3; // PWT
    expected.cr4 ^= 1 << 8; // PCE
    expected.cs.selector ^= 1;
    expected.cs.base ^= 1 << 12;
    expected.cs.limit ^= 1;
    expected.tr.selector ^= 1;
    expected.tr.base ^= 8;
    expected.tr.limit ^= 1;
    expected.ldt.selector ^= 1;
    expected.ldt.base ^= 8;
    expected.ldt.limit ^= 1;
    expected.gdt.base ^= 8;
    expected.gdt.limit ^= 1;
    expected.idt.base ^= 8;
    expected.idt.limit ^= 1;
    expected.sysenter_cs ^= 1;
    expected.sysenter_esp ^= 8;
    expected.sysenter_eip ^= 8;
    expected.msr_efer ^= 1; // SCE
    expected.efer = expected.msr_efer;
    expected.msr_star ^= 1 << 32;
    expected.msr_lstar ^= 1;

    restore
        .drv
        .write_registers(vcpu, Registers::X86(expected.clone()))
        .unwrap();
    assert_eq!(x86_registers(restore.drv.as_ref(), vcpu), expected);

    let vbox = vboxmanage(&[
        "debugvm",
        &vm_name(),
        "getregisters",
        "--cpu",
        &vcpu.to_string(),
        "r12",
    ]);
    assert_eq!(vbox.trim(), format!("r12 = {:#018x}", expected.r12));

    restore.finish();
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_write_registers_reads_back",
    test_fn: write_registers_reads_back
});

/// A register write reaches the vCPU itself, not only VirtualBox's copy of its state: SYSENTER_ESP
/// and SYSENTER_EIP (set once per CPU at bring-up, then unused by an idle 64-bit Linux guest)
/// written while paused keep their new values after the guest ran, and the guest keeps running.
/// The guard then restores only them: the other registers moved on while the guest ran.
fn write_registers_survive_resume(drv: Box<dyn Introspectable>) {
    // a cold-booted guest (VBOX_TEST_CPUS, VBOX_TEST_MEMORY_MB) may not have brought up the vCPU
    let last = drv.get_vcpu_count().unwrap() - 1;
    for _ in 0..60 {
        if x86_registers(drv.as_ref(), last).sysenter_eip != 0 {
            break;
        }
        sleep(Duration::from_millis(500));
    }
    let mut restore = RegisterRestore::new(drv);
    assert_ne!(
        restore.original.sysenter_eip, 0,
        "the guest never set SYSENTER_EIP"
    );
    let vcpu = restore.vcpu;
    let mut changed = restore.original.clone();
    changed.sysenter_esp ^= 8;
    changed.sysenter_eip ^= 8;
    restore
        .drv
        .write_registers(vcpu, Registers::X86(changed.clone()))
        .unwrap();
    restore.drv.resume().unwrap();
    sleep(Duration::from_secs(1));
    let state = vm_info("VMState");
    restore.drv.pause().unwrap();
    let after = x86_registers(restore.drv.as_ref(), vcpu);
    restore.original = X86Registers {
        sysenter_esp: restore.original.sysenter_esp,
        sysenter_eip: restore.original.sysenter_eip,
        ..after.clone()
    };
    assert_eq!(state, "running");
    assert_eq!(
        (after.sysenter_esp, after.sysenter_eip),
        (changed.sysenter_esp, changed.sysenter_eip)
    );
    restore.finish();
}

inventory::submit!(IntegrationTest {
    name: "virtualbox_write_registers_survive_resume",
    test_fn: write_registers_survive_resume
});
