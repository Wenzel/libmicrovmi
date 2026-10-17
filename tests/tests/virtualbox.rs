//! VirtualBox tests checking the driver against ground truth from `VBoxManage`, for any access method.
use std::convert::TryInto;
use std::env;
use std::fs::{self, File};
#[cfg(feature = "virtualbox-vmi")]
use std::io::{self, BufRead, BufReader, Read, Write};
#[cfg(feature = "virtualbox-vmi")]
use std::net::TcpStream;
use std::ops::Range;
use std::os::unix::fs::FileExt;
#[cfg(feature = "virtualbox-vmi")]
use std::process::{Child, Command, Stdio};
#[cfg(feature = "virtualbox-vmi")]
use std::sync::mpsc;
#[cfg(feature = "virtualbox-vmi")]
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use log::debug;
#[cfg(feature = "virtualbox-vmi")]
use microvmi::api::events::{CrType, Event, EventReplyType, EventType, InterceptType};
use microvmi::api::registers::{Registers, X86Registers};
use microvmi::api::{
    Access, Introspectable, DEFAULT_MEMORY_VIEW, KEEP_MEMORY_VIEW, PAGE_SIZE,
};

#[cfg(feature = "virtualbox-vmi")]
use super::next_event;
use super::IntegrationTest;
use crate::common::virtualbox::{core_path, try_vboxmanage, vboxmanage, vm_info, vm_name};
#[cfg(feature = "virtualbox-vmi")]
use crate::common::virtualbox::{start_guest_process, vboxvmi_socket};

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

#[cfg(feature = "virtualbox-vmi")]
fn connect_vboxvmi() -> vboxvmi::Client {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match vboxvmi::Client::connect(vboxvmi_socket()) {
            Ok(client) => return client,
            Err(error) if Instant::now() < deadline => {
                debug!("waiting for the prior VBoxVMI session to close: {}", error);
                sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("failed to connect to VBoxVMI: {}", error),
        }
    }
}

#[cfg(feature = "virtualbox-vmi")]
fn wait_for_vm_state(expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = vm_info("VMState");
        if state == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "VM did not become {} after VBoxVMI disconnect; state is {}",
            expected,
            state
        );
        sleep(Duration::from_millis(20));
    }
}

/// Entry point for the subprocess killed by `vboxvmi_killed_client_resumes`.
#[cfg(feature = "virtualbox-vmi")]
pub fn vboxvmi_pause_child() -> ! {
    let mut client = connect_vboxvmi();
    client
        .pause()
        .expect("child failed to pause through VBoxVMI");
    println!("paused");
    std::io::stdout()
        .flush()
        .expect("child failed to flush readiness");
    loop {
        sleep(Duration::from_secs(60));
    }
}

/// The Intercept a parked child sets on vCPU 0, named by `MICROVMI_VBOXVMI_PARKED_CHILD`.
#[cfg(feature = "virtualbox-vmi")]
fn parked_child_intercept(name: &str) -> InterceptType {
    match name {
        "singlestep" => InterceptType::Singlestep,
        "cr3" => InterceptType::Cr(CrType::Cr3),
        "msr" => InterceptType::Msr(TEST_MSR),
        "pagefault" => InterceptType::Pagefault,
        other => panic!("unknown parked child Intercept {:?}", other),
    }
}

/// Entry point for a killed client with vCPU 0 parked at an event of the Intercept named by
/// `MICROVMI_VBOXVMI_PARKED_CHILD`.
#[cfg(feature = "virtualbox-vmi")]
pub fn vboxvmi_parked_child() -> ! {
    let intercept = parked_child_intercept(&env::var("MICROVMI_VBOXVMI_PARKED_CHILD").unwrap());
    let mut drv = microvmi::init(
        Some(microvmi::api::DriverType::VirtualBox),
        Some(microvmi::api::params::DriverInitParams {
            virtualbox: Some(microvmi::api::params::VBoxInitParams::VBoxVMI {
                socket_path: Some(vboxvmi_socket()),
            }),
            ..Default::default()
        }),
    )
    .expect("child failed to connect");
    // a page-access event needs a restriction: the page of RIP, printed after the readiness line
    let page = match intercept {
        InterceptType::Pagefault => Some(restrict_rip_page(drv.as_mut(), Access::RW)),
        _ => {
            drv.toggle_intercept(0, intercept, true).unwrap();
            None
        }
    };
    let event = next_event(drv.as_mut(), &mut start_guest_process);
    let expected = match (intercept, &event.kind) {
        (InterceptType::Singlestep, EventType::Singlestep { .. }) => true,
        (InterceptType::Cr(cr), EventType::Cr { cr_type, .. }) => *cr_type == cr,
        (InterceptType::Msr(index), EventType::Msr { msr_type, .. }) => *msr_type == index,
        (InterceptType::Pagefault, EventType::Pagefault { .. }) => true,
        _ => false,
    };
    assert!(
        expected,
        "child: {:?} event for {:?}",
        event.kind, intercept
    );
    println!("parked");
    if let Some(page) = page {
        println!("{:#x}", page);
    }
    std::io::stdout()
        .flush()
        .expect("child failed to flush readiness");
    loop {
        sleep(Duration::from_secs(60));
    }
}

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_normal_close_resumes(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    drop(drv);
    wait_for_vm_state("running");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_normal_close_resumes",
    test_fn: vboxvmi_normal_close_resumes
});

#[cfg(feature = "virtualbox-vmi")]
struct ChildGuard(Option<Child>);

#[cfg(feature = "virtualbox-vmi")]
impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }

    fn kill_and_wait(&mut self) -> io::Result<()> {
        {
            let child = self.0.as_mut().unwrap();
            child.kill()?;
            child.wait()?;
        }
        self.0 = None;
        Ok(())
    }
}

#[cfg(feature = "virtualbox-vmi")]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_killed_client_resumes(drv: Box<dyn Introspectable>) {
    drop(drv);
    let child = Command::new(std::env::current_exe().unwrap())
        .env("MICROVMI_VBOXVMI_PAUSE_CHILD", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start VBoxVMI pause child");
    let mut child = ChildGuard::new(child);
    let mut ready = String::new();
    BufReader::new(child.child_mut().stdout.take().unwrap())
        .read_line(&mut ready)
        .expect("failed to read child readiness");
    assert_eq!(
        ready.trim_end(),
        "paused",
        "VBoxVMI pause child did not become ready"
    );
    assert_eq!(vm_info("VMState"), "paused");
    child
        .kill_and_wait()
        .expect("failed to kill and reap VBoxVMI pause child");
    wait_for_vm_state("running");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_killed_client_resumes",
    test_fn: vboxvmi_killed_client_resumes
});

/// A killed client with vCPU 0 parked at an event of `intercept` (see `parked_child_intercept`)
/// leaves the vCPU released and no Intercepts for the next client.
#[cfg(feature = "virtualbox-vmi")]
fn killed_client_releases_parked_vcpu(drv: Box<dyn Introspectable>, intercept: &str) {
    drop(drv);
    let child = Command::new(std::env::current_exe().unwrap())
        .env("MICROVMI_VBOXVMI_PARKED_CHILD", intercept)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start parked client");
    let mut child = ChildGuard::new(child);
    let mut stdout = BufReader::new(child.child_mut().stdout.take().unwrap());
    let mut ready = String::new();
    stdout
        .read_line(&mut ready)
        .expect("failed to read child readiness");
    assert_eq!(ready.trim_end(), "parked", "child did not park vCPU");
    let mut page = String::new();
    if intercept == "pagefault" {
        stdout
            .read_line(&mut page)
            .expect("failed to read child page");
    }
    let before = vcpu_stats("cNsExecuting")[0];
    sleep(Duration::from_millis(200));
    assert_eq!(vcpu_stats("cNsExecuting")[0], before);
    child.kill_and_wait().expect("failed to kill parked client");
    drop(connect_vboxvmi()); // accepting the next client proves disconnect cleanup finished
    let mut drv = microvmi::init(
        Some(microvmi::api::DriverType::VirtualBox),
        Some(microvmi::api::params::DriverInitParams {
            virtualbox: Some(microvmi::api::params::VBoxInitParams::VBoxVMI {
                socket_path: Some(vboxvmi_socket()),
            }),
            ..Default::default()
        }),
    )
    .unwrap();
    if intercept == "pagefault" {
        let page = u64::from_str_radix(page.trim().trim_start_matches("0x"), 16).unwrap();
        assert_eq!(
            drv.get_page_access(DEFAULT_MEMORY_VIEW, page).unwrap(),
            Access::RWX,
            "page access restriction left after disconnect"
        );
    }
    assert_no_event(
        drv.as_mut(),
        &mut start_guest_process,
        &format!("stale {} event after disconnect", intercept),
    );
    let resumed = vcpu_stats("cNsExecuting")[0];
    sleep(Duration::from_millis(500));
    assert!(
        vcpu_stats("cNsExecuting")[0] > resumed,
        "vCPU stayed parked after disconnect"
    );
}

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_killed_client_releases_parked_vcpu(drv: Box<dyn Introspectable>) {
    killed_client_releases_parked_vcpu(drv, "singlestep");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_killed_client_releases_parked_vcpu",
    test_fn: vboxvmi_killed_client_releases_parked_vcpu
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_killed_client_releases_cr_parked_vcpu(drv: Box<dyn Introspectable>) {
    killed_client_releases_parked_vcpu(drv, "cr3");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_killed_client_releases_cr_parked_vcpu",
    test_fn: vboxvmi_killed_client_releases_cr_parked_vcpu
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_killed_client_releases_msr_parked_vcpu(drv: Box<dyn Introspectable>) {
    killed_client_releases_parked_vcpu(drv, "msr");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_killed_client_releases_msr_parked_vcpu",
    test_fn: vboxvmi_killed_client_releases_msr_parked_vcpu
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_killed_client_releases_page_access_parked_vcpu(drv: Box<dyn Introspectable>) {
    killed_client_releases_parked_vcpu(drv, "pagefault");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_killed_client_releases_page_access_parked_vcpu",
    test_fn: vboxvmi_killed_client_releases_page_access_parked_vcpu
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_external_pause_is_not_resumed(mut drv: Box<dyn Introspectable>) {
    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    drv.pause().unwrap();
    drop(drv);
    drop(connect_vboxvmi()); // accepting the next client proves disconnect cleanup finished
    assert_eq!(vm_info("VMState"), "paused");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_preserves_preexisting_vm_state",
    test_fn: vboxvmi_external_pause_is_not_resumed
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_resume_generation_preserves_external_pause(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    vboxmanage(&["controlvm", &vm_name(), "resume"]);
    vboxmanage(&["controlvm", &vm_name(), "pause"]);
    drop(drv);
    drop(connect_vboxvmi()); // accepting the next client proves disconnect cleanup finished
    assert_eq!(vm_info("VMState"), "paused");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_preserves_new_external_vm_state",
    test_fn: vboxvmi_resume_generation_preserves_external_pause
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_disconnect_race_preserves_external_state(drv: Box<dyn Introspectable>) {
    drop(drv);
    for iteration in 0..8 {
        if iteration != 0 {
            vboxmanage(&["controlvm", &vm_name(), "resume"]);
        }
        let mut client = connect_vboxvmi();
        client.pause().unwrap();

        let (resumed_tx, resumed_rx) = mpsc::channel();
        let external = thread::spawn(move || {
            let _ = try_vboxmanage(&["controlvm", &vm_name(), "resume"]);
            resumed_tx.send(()).unwrap();
            vboxmanage(&["controlvm", &vm_name(), "pause"]);
        });
        if iteration % 2 != 0 {
            resumed_rx.recv().unwrap();
        } else {
            thread::yield_now();
        }
        drop(client);
        external.join().expect("external resume/pause panicked");

        drop(connect_vboxvmi()); // accepting the next client proves disconnect cleanup finished
        assert_eq!(
            vm_info("VMState"),
            "paused",
            "disconnect resumed external pause in race iteration {}",
            iteration
        );
    }
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_race_preserves_external_state",
    test_fn: vboxvmi_disconnect_race_preserves_external_state
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_second_client_is_busy(drv: Box<dyn Introspectable>) {
    match vboxvmi::Client::connect(vboxvmi_socket()) {
        Err(vboxvmi::Error::Busy) => {}
        Err(error) => panic!("second VBoxVMI client got the wrong error: {}", error),
        Ok(_) => panic!("second VBoxVMI client connected"),
    }
    drop(drv);
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_disconnect_second_client_busy",
    test_fn: vboxvmi_second_client_is_busy
});

/// The default access method takes VBoxVMI when the VM serves its socket, and MachineDebugger only
/// when the socket file is missing; a busy socket, or an explicit VBoxVMI without the socket, is an
/// error. Meaningful under every `VBOX_TEST_METHOD`: it only relies on whether VBoxVMI is enabled.
#[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
fn default_method_selection(drv: Box<dyn Introspectable>) {
    use crate::common::virtualbox::extradata;
    use microvmi::api::params::{CommonInitParams, DriverInitParams, VBoxInitParams};
    use microvmi::api::DriverType;

    let init = |virtualbox| {
        microvmi::init(
            Some(DriverType::VirtualBox),
            Some(DriverInitParams {
                common: Some(CommonInitParams { vm_name: vm_name() }),
                virtualbox,
                ..Default::default()
            }),
        )
    };
    if extradata("VBoxInternal/VMI/Enabled").as_deref() == Some("1") {
        // Fails only if `drv` holds the one VBoxVMI client slot, and the default didn't fall back.
        let err = init(None)
            .err()
            .expect("default init succeeded while the VBoxVMI socket is busy");
        assert!(
            err.to_string().contains("already connected"),
            "default init: {}",
            err
        );
    } else {
        assert!(!std::path::Path::new(&vboxvmi_socket()).exists());
        let (mut byte, mut read) = ([0u8], 0);
        drv.read_physical(0, &mut byte, &mut read)
            .expect("MachineDebugger read");
        let err = init(Some(VBoxInitParams::VBoxVMI { socket_path: None }))
            .err()
            .expect("explicit VBoxVMI without a socket fell back");
        assert!(
            err.to_string()
                .contains("cannot connect to the VBoxVMI socket"),
            "explicit VBoxVMI: {}",
            err
        );
    }
    drop(drv);
}

#[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
inventory::submit!(IntegrationTest {
    name: "virtualbox_default_method_selection",
    test_fn: default_method_selection
});

/// Resumes the VM even if selecting pages or dumping the first core fails.
struct ResumeVm;

impl Drop for ResumeVm {
    fn drop(&mut self) {
        let vm = vm_name();
        let _ = try_vboxmanage(&["controlvm", &vm, "resume"]);
    }
}

/// Restores original page contents before the VM is resumed, including during unwinding.
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
    let _resume = ResumeVm;
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

/// Restores a patched instruction before the VBoxVMI session is dropped, even if a test fails
/// while its vCPU is parked.
#[cfg(feature = "virtualbox-vmi")]
struct BreakpointRestore {
    drv: Box<dyn Introspectable>,
    gpa: u64,
    original: u8,
    patched: bool,
    intercepted: bool,
    paused: bool,
}

#[cfg(feature = "virtualbox-vmi")]
impl Drop for BreakpointRestore {
    fn drop(&mut self) {
        if self.patched && !self.paused {
            match self.drv.pause() {
                Ok(()) => self.paused = true,
                Err(error) => eprintln!("failed to pause VM before restoring int3: {}", error),
            }
        }
        if self.patched {
            if let Err(error) = self.drv.write_physical(self.gpa, &[self.original]) {
                eprintln!("failed to restore int3 at {:#x}: {}", self.gpa, error);
            }
        }
        if self.intercepted {
            if let Err(error) = self
                .drv
                .toggle_intercept(0, InterceptType::Breakpoint, false)
            {
                eprintln!("failed to clear breakpoint intercept: {}", error);
            }
        }
        if self.paused {
            if let Err(error) = self.drv.resume() {
                eprintln!("failed to resume VM after breakpoint test: {}", error);
            }
        }
    }
}

/// The vCPU is paused at RIP before the int3 is written. Restoring the byte before the reply
/// makes a non-reinjecting reply execute the original instruction.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_breakpoint_at_rip(mut drv: Box<dyn Introspectable>) {
    assert_eq!(
        drv.get_vcpu_count().unwrap(),
        1,
        "breakpoint test requires one vCPU"
    );
    drv.pause().unwrap();
    let before = x86_registers(drv.as_ref(), 0);
    let gpa = translate(drv.as_ref(), before.cr3, before.rip).expect("RIP is not mapped");
    let mut original = [0];
    let mut bytes_read = 0;
    drv.read_physical(gpa, &mut original, &mut bytes_read)
        .unwrap();
    assert_eq!(bytes_read, 1);
    assert_ne!(original[0], 0xcc, "RIP was already an int3");
    let mut restore = BreakpointRestore {
        drv,
        gpa,
        original: original[0],
        patched: true,
        intercepted: true,
        paused: true,
    };
    restore.drv.write_physical(gpa, &[0xcc]).unwrap();
    restore
        .drv
        .toggle_intercept(0, InterceptType::Breakpoint, true)
        .unwrap();
    restore.drv.resume().unwrap();
    restore.paused = false;
    let event = restore.drv.listen(5000).unwrap().expect("no int3 at RIP");
    assert_eq!(event.vcpu, 0);
    assert!(matches!(event.kind, EventType::Breakpoint { gpa: addr, insn_len: 1 } if addr == gpa));
    assert_eq!(x86_registers(restore.drv.as_ref(), 0).rip, before.rip);
    restore.drv.write_physical(gpa, &original).unwrap();
    restore.patched = false;
    restore
        .drv
        .toggle_intercept(0, InterceptType::Breakpoint, false)
        .unwrap();
    restore.intercepted = false;
    restore
        .drv
        .reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let before_resume = vcpu_stats("cNsExecuting")[0];
    sleep(Duration::from_millis(500));
    assert!(
        vcpu_stats("cNsExecuting")[0] > before_resume,
        "vCPU did not run after breakpoint reply"
    );
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_breakpoint_at_rip",
    test_fn: vboxvmi_breakpoint_at_rip
});

/// A step arrives at every instruction boundary on one vCPU. Changing intercepts while that
/// vCPU is parked must affect the instruction executed on reply.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_singlestep(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    drv.resume().unwrap();
    let mut previous_rip = None;
    let mut advanced = false;
    for _ in 0..12 {
        let event = drv.listen(5000).unwrap().expect("no single-step event");
        let regs = x86_registers(drv.as_ref(), 0);
        let gpa = translate(drv.as_ref(), regs.cr3, regs.rip).expect("step RIP is not mapped");
        assert!(matches!(event.kind, EventType::Singlestep { gpa: addr } if addr == gpa));
        assert_eq!(event.vcpu, 0);
        if let Some(last) = previous_rip {
            advanced |= last != regs.rip;
        }
        previous_rip = Some(regs.rip);
        drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
            .unwrap();
    }
    assert!(advanced, "RIP never advanced while stepping");
    let event = drv.listen(5000).unwrap().expect("no event to clear from");
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert!(drv.listen(250).unwrap().is_none(), "step after clear");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_singlestep",
    test_fn: vboxvmi_singlestep
});

/// Interrupts are delivered while stepping: a step lands on the first instruction of an external
/// interrupt handler (an IDT gate target for vectors 32-255) on a guest taking timer interrupts.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_singlestep_enters_interrupt_handler(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    let regs = x86_registers(drv.as_ref(), 0);
    let idt = translate(drv.as_ref(), regs.cr3, regs.idt.base).expect("IDT is not mapped");
    let mut gates = vec![0; 256 * 16];
    let mut bytes_read = 0;
    drv.read_physical(idt, &mut gates, &mut bytes_read).unwrap();
    // 64-bit gate: offset 15:0 at 0, 31:16 at 6, 63:32 at 8
    let handlers: Vec<u64> = gates
        .chunks(16)
        .skip(32)
        .map(|g| {
            u64::from(u16::from_le_bytes([g[0], g[1]]))
                | u64::from(u16::from_le_bytes([g[6], g[7]])) << 16
                | u64::from(u32::from_le_bytes(g[8..12].try_into().unwrap())) << 32
        })
        .collect();
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    drv.resume().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut steps = 0;
    let entered = loop {
        let event = drv.listen(5000).unwrap().expect("no single-step event");
        steps += 1;
        let rip = x86_registers(drv.as_ref(), 0).rip;
        let entered = handlers.contains(&rip);
        let done = entered || Instant::now() > deadline;
        if done {
            drv.toggle_intercept(0, InterceptType::Singlestep, false)
                .unwrap();
        }
        drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
            .unwrap();
        if done {
            break entered;
        }
    };
    assert!(
        entered,
        "no step entered an interrupt handler in {} steps",
        steps
    );
    print!("interrupt handler entered after {} steps ", steps);
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_singlestep_enters_interrupt_handler",
    test_fn: vboxvmi_singlestep_enters_interrupt_handler
});

/// Connects to the VM's debugger console (enabled by the VBoxVMI context, default port 5000),
/// which attaches the VirtualBox debugger, and returns it with its output up to the prompt (attached)
/// or the attach error.
#[cfg(feature = "virtualbox-vmi")]
fn debugger_console() -> (TcpStream, String) {
    let mut console =
        TcpStream::connect("127.0.0.1:5000").expect("no debugger console on port 5000");
    console
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut output = Vec::new();
    let mut buf = [0; 1024];
    // an attach error is the line after "error:", which may arrive in pieces
    let attach_error = |out: &[u8]| {
        out.windows(6)
            .position(|w| w == b"error:")
            .is_some_and(|i| out[i..].contains(&b'\n'))
    };
    while !(output.ends_with(b"VBoxDbg> ") || attach_error(&output)) {
        match console.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => output.extend_from_slice(&buf[..n]),
        }
    }
    (console, String::from_utf8_lossy(&output).into_owned())
}

/// VBoxVMI Intercepts and the VirtualBox debugger exclude each other (ADR-0005): the debugger
/// can't attach while an Intercept (`set`) is set, an Intercept (`while_attached`) can't be set
/// while it is attached, and either works again once the other is gone.
#[cfg(feature = "virtualbox-vmi")]
fn debugger_exclusion(
    mut drv: Box<dyn Introspectable>,
    set: InterceptType,
    while_attached: InterceptType,
) {
    drv.toggle_intercept(0, set, true).unwrap();
    let (_, output) = debugger_console();
    assert!(
        output.contains("VERR_DBGF_VMI_INTERCEPTS_ACTIVE"),
        "debugger attached with an Intercept set: {:?}",
        output
    );
    drv.toggle_intercept(0, set, false).unwrap();

    let (console, output) = debugger_console();
    assert!(
        output.ends_with("VBoxDbg> "),
        "debugger did not attach: {:?}",
        output
    );
    let error = drv
        .toggle_intercept(0, while_attached, true)
        .expect_err("Intercept set with the debugger attached");
    assert!(error.to_string().contains("debugger"), "{}", error);

    // closing the console detaches the debugger, asynchronously
    drop(console);
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Err(e) = drv.toggle_intercept(0, set, true) {
        assert!(Instant::now() < deadline, "debugger still attached: {}", e);
        sleep(Duration::from_millis(100));
    }
    drv.toggle_intercept(0, set, false).unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_debugger_exclusion(drv: Box<dyn Introspectable>) {
    debugger_exclusion(drv, InterceptType::Breakpoint, InterceptType::Singlestep);
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_debugger_exclusion",
    test_fn: vboxvmi_debugger_exclusion
});

#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr_debugger_exclusion(drv: Box<dyn Introspectable>) {
    let cr3 = InterceptType::Cr(CrType::Cr3);
    debugger_exclusion(drv, cr3, cr3);
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_cr_debugger_exclusion",
    test_fn: vboxvmi_cr_debugger_exclusion
});

/// A parked vCPU serves register reads/writes; pause and resume do not answer its event.
/// An invalid reply must not release it.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_parked_vcpu(mut drv: Box<dyn Introspectable>) {
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    let event = drv.listen(5000).unwrap().expect("no step");
    assert!(matches!(event.kind, EventType::Singlestep { .. }));
    let regs = x86_registers(drv.as_ref(), 0);
    let mut changed = regs.clone();
    changed.rax ^= 0x5a5a;
    drv.write_registers(0, Registers::X86(changed.clone()))
        .unwrap();
    assert_eq!(x86_registers(drv.as_ref(), 0).rax, changed.rax);
    drv.pause().unwrap();
    drv.resume().unwrap();
    assert_eq!(x86_registers(drv.as_ref(), 0).rip, regs.rip);
    if drv.get_vcpu_count().unwrap() > 1 {
        assert!(
            drv.reply_event(
                Event {
                    vcpu: 1,
                    view: DEFAULT_MEMORY_VIEW,
                    kind: EventType::Singlestep { gpa: 0 }
                },
                EventReplyType::Continue {
                    reinject: false,
                    view: KEEP_MEMORY_VIEW,
                }
            )
            .is_err(),
            "reply to an unparked vCPU succeeded"
        );
    }
    let vcpu = drv.get_vcpu_count().unwrap();
    assert!(drv
        .reply_event(
            Event {
                vcpu,
                view: DEFAULT_MEMORY_VIEW,
                kind: EventType::Singlestep { gpa: 0 }
            },
            EventReplyType::Continue {
                reinject: false,
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    assert!(drv
        .reply_event(
            Event {
                vcpu: 0,
                view: DEFAULT_MEMORY_VIEW,
                kind: EventType::Singlestep { gpa: 0 }
            },
            EventReplyType::Continue {
                reinject: true,
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    assert_eq!(x86_registers(drv.as_ref(), 0).rip, regs.rip);
    drv.write_registers(0, Registers::X86(regs)).unwrap();
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.pause().unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    assert!(drv
        .reply_event(
            Event {
                vcpu: 0,
                view: DEFAULT_MEMORY_VIEW,
                kind: EventType::Singlestep { gpa: 0 }
            },
            EventReplyType::Continue {
                reinject: false,
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    drv.resume().unwrap();
    assert!(drv.listen(250).unwrap().is_none());
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_parked_vcpu",
    test_fn: vboxvmi_parked_vcpu
});

/// Control register number of `cr`.
#[cfg(feature = "virtualbox-vmi")]
fn cr_number(cr: CrType) -> u8 {
    match cr {
        CrType::Cr0 => 0,
        CrType::Cr3 => 3,
        CrType::Cr4 => 4,
    }
}

/// `cr`'s value in `regs`.
#[cfg(feature = "virtualbox-vmi")]
fn cr_value(regs: &X86Registers, cr: CrType) -> u64 {
    match cr {
        CrType::Cr0 => regs.cr0,
        CrType::Cr3 => regs.cr3,
        CrType::Cr4 => regs.cr4,
    }
}

/// Old and new value of a write event to `cr`.
#[cfg(feature = "virtualbox-vmi")]
fn cr_event(event: &Event, cr: CrType) -> (u64, u64) {
    match event.kind {
        EventType::Cr { cr_type, new, old } if cr_type == cr => (old, new),
        ref other => panic!("not a {:?} write event: {:?}", cr, other),
    }
}

/// The first `len` bytes at `regs.rip`, read through the page tables at `regs.cr3`.
#[cfg(feature = "virtualbox-vmi")]
fn code_at_rip(drv: &dyn Introspectable, regs: &X86Registers, len: u64) -> Vec<u8> {
    (regs.rip..regs.rip + len)
        .map(|vaddr| {
            let gpa = translate(drv, regs.cr3, vaddr)
                .unwrap_or_else(|| panic!("instruction byte {:#x} is not mapped", vaddr));
            let mut byte = [0];
            let mut bytes_read = 0;
            drv.read_physical(gpa, &mut byte, &mut bytes_read).unwrap();
            byte[0]
        })
        .collect()
}

/// Whether `code` starts with `MOV CRn, r64`: optional REX prefix, `0f 22`, ModRM with mod 11
/// and reg `n`.
#[cfg(feature = "virtualbox-vmi")]
fn is_mov_to_cr(code: &[u8], n: u8) -> bool {
    let code = match code[0] {
        0x40..=0x4f => &code[1..],
        _ => code,
    };
    code[..2] == [0x0f, 0x22] && code[2] >> 6 == 3 && (code[2] >> 3) & 7 == n
}

/// No event within 2 s of `activity`.
#[cfg(feature = "virtualbox-vmi")]
fn assert_no_event(drv: &mut dyn Introspectable, activity: &mut dyn FnMut(), what: &str) {
    activity();
    if let Some(event) = drv.listen(2000).unwrap() {
        panic!("{}: {:?} on vCPU {}", what, event.kind, event.vcpu);
    }
}

/// `events` write events to `cr`, with its Intercept on vCPU 0 only and `activity` making the
/// guest write it; returns their old and new values. Each is reported before the write (the
/// register still holds the old value) with RIP at a `MOV CRn`. Each old value is the previous
/// event's new one, so the write proceeded with the new value after the reply (not checked for
/// CR0: `CLTS`/`LMSW` are not reported). Reinjection is refused and leaves the vCPU parked. No
/// event from a vCPU without the Intercept (needs 2 vCPUs); none after clearing it from within
/// event handling.
#[cfg(feature = "virtualbox-vmi")]
fn cr_write_events(
    drv: &mut dyn Introspectable,
    cr: CrType,
    events: usize,
    activity: &mut dyn FnMut(),
) -> Vec<(u64, u64)> {
    let n = cr_number(cr);
    drv.toggle_intercept(0, InterceptType::Cr(cr), true)
        .unwrap();
    let mut values: Vec<(u64, u64)> = Vec::new();
    for _ in 0..events {
        let event = next_event(drv, activity);
        assert_eq!(event.vcpu, 0, "CR event from a vCPU without the Intercept");
        let (old, new) = cr_event(&event, cr);
        let regs = x86_registers(drv, 0);
        assert_eq!(
            old,
            cr_value(&regs, cr),
            "old value is not CR{} at the event",
            n
        );
        if cr == CrType::Cr3 {
            assert_eq!(new >> 63, 0, "CR3 new value {:#x} has bit 63 set", new);
        }
        if let (Some(&(_, previous)), true) = (values.last(), cr != CrType::Cr0) {
            assert_eq!(
                old, previous,
                "CR{} is not the previous write's new value",
                n
            );
        }
        let code = code_at_rip(drv, &regs, 4);
        assert!(
            is_mov_to_cr(&code, n),
            "RIP {:#x} is not a MOV CR{}: {:02x?}",
            regs.rip,
            n,
            code
        );
        let reinjected = drv.reply_event(
            Event { vcpu: 0, view: DEFAULT_MEMORY_VIEW, kind: EventType::Cr {
                cr_type: cr,
                new,
                old,
            } },
            EventReplyType::Continue { reinject: true, view: KEEP_MEMORY_VIEW },
        );
        assert!(reinjected.is_err(), "CR event reinjected");
        let parked = x86_registers(drv, 0);
        assert_eq!((parked.rip, cr_value(&parked, cr)), (regs.rip, old));
        values.push((old, new));
        drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
            .unwrap();
    }
    let event = next_event(drv, activity);
    assert_eq!(event.vcpu, 0, "CR event from a vCPU without the Intercept");
    drv.toggle_intercept(0, InterceptType::Cr(cr), false)
        .unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv, activity, "CR event after clear");
    values
}

/// CR3 writes of the processes `start_guest_process` starts.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr3_write(mut drv: Box<dyn Introspectable>) {
    cr_write_events(drv.as_mut(), CrType::Cr3, 8, &mut start_guest_process);
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_cr3_write",
    test_fn: vboxvmi_cr3_write
});

/// Runs `command` in the guest through `VBOX_TEST_GUEST_SSH`, an ssh command line into it,
/// without waiting: the command may need the vCPU a test parks.
#[cfg(feature = "virtualbox-vmi")]
fn guest_command(command: &str) -> ChildGuard {
    let ssh = env::var("VBOX_TEST_GUEST_SSH")
        .expect("set VBOX_TEST_GUEST_SSH to an ssh command line into the test guest");
    let mut ssh = ssh.split_whitespace();
    let child = Command::new(ssh.next().expect("empty VBOX_TEST_GUEST_SSH"))
        .args(ssh)
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start ssh");
    ChildGuard::new(child)
}

#[cfg(feature = "virtualbox-vmi")]
const CR4_TSD: u64 = 1 << 2;

/// A process on vCPU 0 with `PR_SET_TSC` set to `PR_TSC_SIGSEGV` (prctl 26, 2) sleeping in a
/// loop: Linux flips CR4.TSD on every switch to and from it.
#[cfg(feature = "virtualbox-vmi")]
const CR4_TSD_COMMAND: &str = "taskset -c 0 perl -e 'syscall(157, 26, 2, 0, 0, 0) == 0 or die; select(undef, undef, undef, 0.01) for 1..500'";

/// CR4 writes, including writes changing only CR4.TSD, a bit VirtualBox leaves to the guest.
/// Needs the Ubuntu guest (`VBOX_TEST_VM=microvmi-ubuntu`) and `VBOX_TEST_GUEST_SSH`.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr4_write(mut drv: Box<dyn Introspectable>) {
    let mut ssh = Vec::new();
    let values = cr_write_events(drv.as_mut(), CrType::Cr4, 16, &mut || {
        ssh.push(guest_command(CR4_TSD_COMMAND))
    });
    assert!(
        values.iter().any(|(old, new)| old ^ new == CR4_TSD),
        "no write changing only CR4.TSD: {:x?}",
        values
    );
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_ssh_events_cr4_write",
    test_fn: vboxvmi_cr4_write
});

/// Adds and removes a write-combining MTRR: every CPU disables its caches meanwhile (CR0.CD).
#[cfg(feature = "virtualbox-vmi")]
const CR0_CD_COMMAND: &str = r#"sudo sh -c 'echo base=0xf0000000 size=0x100000 type=write-combining >/proc/mtrr; r=$(sed -n "s/^reg0*\([0-9][0-9]*\): base=0x0*f0000000 .*/\1/p" /proc/mtrr); echo disable=$r >/proc/mtrr'"#;

#[cfg(feature = "virtualbox-vmi")]
const CR0_CD: u64 = 1 << 30;

/// CR0 writes: setting and clearing CR0.CD. Needs the Ubuntu guest
/// (`VBOX_TEST_VM=microvmi-ubuntu`) and `VBOX_TEST_GUEST_SSH`.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr0_write(mut drv: Box<dyn Introspectable>) {
    let mut ssh = Vec::new();
    let values = cr_write_events(drv.as_mut(), CrType::Cr0, 4, &mut || {
        ssh.push(guest_command(CR0_CD_COMMAND))
    });
    assert!(
        values.iter().any(|(old, new)| old ^ new == CR0_CD),
        "no write toggling CR0.CD: {:x?}",
        values
    );
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_ssh_events_cr0_write",
    test_fn: vboxvmi_cr0_write
});

/// CR Intercepts are set and cleared while paused (no event while paused); a vCPU parked at a
/// CR event stays parked, its write not done, through pause and resume; a reply while paused is
/// accepted and the vCPU continues on resume.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr_paused(mut drv: Box<dyn Introspectable>) {
    let cr3 = InterceptType::Cr(CrType::Cr3);
    drv.pause().unwrap();
    drv.toggle_intercept(0, cr3, true).unwrap();
    assert!(drv.listen(500).unwrap().is_none(), "CR event while paused");
    drv.toggle_intercept(0, cr3, false).unwrap();
    drv.resume().unwrap();
    assert_no_event(
        drv.as_mut(),
        &mut start_guest_process,
        "CR event after clear",
    );

    drv.pause().unwrap();
    drv.toggle_intercept(0, cr3, true).unwrap();
    drv.resume().unwrap();
    let event = next_event(drv.as_mut(), &mut start_guest_process);
    let (old, _) = cr_event(&event, CrType::Cr3);
    let regs = x86_registers(drv.as_ref(), 0);
    drv.pause().unwrap();
    drv.resume().unwrap();
    sleep(Duration::from_millis(200));
    let parked = x86_registers(drv.as_ref(), 0);
    assert_eq!(
        (parked.rip, parked.cr3),
        (regs.rip, old),
        "vCPU left its CR event on pause and resume"
    );
    drv.pause().unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    drv.toggle_intercept(0, cr3, false).unwrap();
    assert_eq!(vm_info("VMState"), "paused");
    drv.resume().unwrap();
    let resumed = vcpu_stats("cNsExecuting")[0];
    sleep(Duration::from_millis(500));
    assert!(
        vcpu_stats("cNsExecuting")[0] > resumed,
        "vCPU did not run after a reply while paused"
    );
    assert_no_event(
        drv.as_mut(),
        &mut start_guest_process,
        "CR event after clear",
    );
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_cr_paused",
    test_fn: vboxvmi_cr_paused
});

/// With single step also set, the CR event comes first, then the single-step event after the
/// write: CR3 then holds the new value.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_cr_before_singlestep(mut drv: Box<dyn Introspectable>) {
    let cr3 = InterceptType::Cr(CrType::Cr3);
    drv.toggle_intercept(0, cr3, true).unwrap();
    let event = next_event(drv.as_mut(), &mut start_guest_process);
    let (_, new) = cr_event(&event, CrType::Cr3);
    let rip = x86_registers(drv.as_ref(), 0).rip;
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let step = drv
        .listen(5000)
        .unwrap()
        .expect("no step after the CR event");
    assert_eq!(step.vcpu, 0);
    assert!(
        matches!(step.kind, EventType::Singlestep { .. }),
        "{:?} instead of the step after the CR event",
        step.kind
    );
    let regs = x86_registers(drv.as_ref(), 0);
    assert_ne!(regs.rip, rip, "step before the MOV CR3 executed");
    assert_eq!(
        regs.cr3, new,
        "CR3 is not the CR event's new value after the step"
    );
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.toggle_intercept(0, cr3, false).unwrap();
    drv.reply_event(step, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv.as_mut(), &mut start_guest_process, "event after clear");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_cr_before_singlestep",
    test_fn: vboxvmi_cr_before_singlestep
});

/// The gva, gpa and access of a page-access event.
#[cfg(feature = "virtualbox-vmi")]
fn page_access_event(event: &Event) -> (u64, u64, Access) {
    match event.kind {
        EventType::Pagefault { gva, gpa, access } => (gva, gpa, access),
        ref other => panic!("{:?} instead of a page-access event", other),
    }
}

/// Restricts the page of vCPU 0's RIP, the idle loop of the idle test guest, which runs again at
/// every timer interrupt, to `allowed` while paused, and sets the page-access Intercept on vCPU 0
/// only. Returns RIP's gpa.
#[cfg(feature = "virtualbox-vmi")]
fn restrict_rip_page(drv: &mut dyn Introspectable, allowed: Access) -> u64 {
    drv.pause().unwrap();
    let regs = x86_registers(drv, 0);
    let gpa = translate(drv, regs.cr3, regs.rip).expect("RIP is not mapped");
    assert_eq!(
        drv.get_page_access(DEFAULT_MEMORY_VIEW, gpa).unwrap(),
        Access::RWX,
        "unrestricted page"
    );
    drv.set_page_access(DEFAULT_MEMORY_VIEW, gpa, allowed).unwrap();
    assert_eq!(
        drv.get_page_access(DEFAULT_MEMORY_VIEW, gpa).unwrap(),
        allowed,
        "get after set"
    );
    drv.toggle_intercept(0, InterceptType::Pagefault, true)
        .unwrap();
    drv.resume().unwrap();
    gpa
}

/// The next event, an execute access of vCPU 0 at RIP, on `page`. Returns it with RIP.
#[cfg(feature = "virtualbox-vmi")]
fn execute_event(drv: &mut dyn Introspectable, page: u64) -> (Event, u64) {
    let event = drv.listen(5000).unwrap().expect("no page-access event");
    assert_eq!(
        event.vcpu, 0,
        "page-access event from a vCPU without the Intercept"
    );
    let (gva, gpa, access) = page_access_event(&event);
    let regs = x86_registers(drv, 0);
    assert_eq!(access, Access::X);
    assert_eq!(gpa / PAGE, page / PAGE, "event outside the restricted page");
    assert_eq!(
        translate(drv, regs.cr3, regs.rip),
        Some(gpa),
        "gpa is not RIP's"
    );
    assert!(
        gva == 0 || gva == regs.rip,
        "gva {:#x}, RIP {:#x}",
        gva,
        regs.rip
    );
    (event, regs.rip)
}

/// Execute restriction on the idle loop: refusals, events at RIP on vCPU 0 only, reinjection and
/// a reply that does not fit refused with the vCPU parked, a plain retry faulting again,
/// `Emulate` running one instruction with the restriction kept, and no event once lifted.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_page_access_execute(mut drv: Box<dyn Introspectable>) {
    assert!(
        drv.set_page_access(DEFAULT_MEMORY_VIEW, 0, Access::W)
            .is_err(),
        "write without read"
    );
    assert!(
        drv.set_page_access(DEFAULT_MEMORY_VIEW, 0, Access::WX)
            .is_err(),
        "write without read"
    );
    assert!(
        drv.set_page_access(DEFAULT_MEMORY_VIEW, 0xfee0_0000, Access::R)
            .is_err(),
        "local APIC (MMIO) page"
    );
    let page = restrict_rip_page(drv.as_mut(), Access::RW);
    let (event, rip) = execute_event(drv.as_mut(), page);
    if drv.get_vcpu_count().unwrap() > 1 {
        let before = vcpu_stats("cNsExecuting")[1];
        sleep(Duration::from_millis(500));
        assert!(
            vcpu_stats("cNsExecuting")[1] > before,
            "vCPU without the Intercept stopped on the restricted page"
        );
    }
    assert!(drv
        .reply_event(
            event.clone(),
            EventReplyType::Continue {
                reinject: true,
                view: KEEP_MEMORY_VIEW,
            },
        )
        .is_err());
    assert!(drv
        .reply_event(
            event.clone(),
            EventReplyType::EmulateInsn {
                data: [0x90; 16],
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    assert_eq!(
        x86_registers(drv.as_ref(), 0).rip,
        rip,
        "vCPU left on a refused reply"
    );
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let (event, retried) = execute_event(drv.as_mut(), page);
    assert_eq!(retried, rip, "retry did not fault again");

    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    drv.reply_event(event, EventReplyType::Emulate { view: KEEP_MEMORY_VIEW }).unwrap();
    let step = drv.listen(5000).unwrap().expect("no step after Emulate");
    assert!(
        matches!(step.kind, EventType::Singlestep { .. }),
        "{:?}",
        step.kind
    );
    assert_ne!(
        x86_registers(drv.as_ref(), 0).rip,
        rip,
        "Emulate did not run the instruction"
    );
    assert_eq!(
        drv.get_page_access(DEFAULT_MEMORY_VIEW, page).unwrap(),
        Access::RW,
        "Emulate lifted the restriction"
    );
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.reply_event(step, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();

    let (event, _) = execute_event(drv.as_mut(), page);
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv.as_mut(), &mut || (), "page-access event after lifting");
    drv.toggle_intercept(0, InterceptType::Pagefault, false)
        .unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_page_access_execute",
    test_fn: vboxvmi_page_access_execute
});

/// An int3 fetched from an execute-restricted page and executed by VirtualBox's emulator still
/// raises a breakpoint event.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_emulated_int3_breakpoint(mut drv: Box<dyn Introspectable>) {
    assert_eq!(
        drv.get_vcpu_count().unwrap(),
        1,
        "breakpoint test requires one vCPU"
    );
    drv.pause().unwrap();
    let regs = x86_registers(drv.as_ref(), 0);
    let gpa = translate(drv.as_ref(), regs.cr3, regs.rip).expect("RIP is not mapped");
    let mut original = [0; 16];
    drv.read_physical(gpa, &mut original, &mut 0).unwrap();
    assert_ne!(original[0], 0xcc, "RIP was already an int3");
    let mut restore = BreakpointRestore {
        drv,
        gpa,
        original: original[0],
        patched: true,
        intercepted: true,
        paused: true,
    };
    restore.drv.write_physical(gpa, &[0xcc]).unwrap();
    restore.drv.set_page_access(DEFAULT_MEMORY_VIEW, gpa, Access::RW).unwrap();
    restore
        .drv
        .toggle_intercept(0, InterceptType::Pagefault, true)
        .unwrap();
    restore
        .drv
        .toggle_intercept(0, InterceptType::Breakpoint, true)
        .unwrap();
    restore.drv.resume().unwrap();
    restore.paused = false;

    let (execute, _) = execute_event(restore.drv.as_mut(), gpa);
    restore
        .drv
        .reply_event(execute, EventReplyType::Emulate { view: KEEP_MEMORY_VIEW })
        .unwrap();
    let breakpoint = restore
        .drv
        .listen(5000)
        .unwrap()
        .expect("emulated int3 did not raise a breakpoint event");
    assert!(matches!(
        breakpoint.kind,
        EventType::Breakpoint {
            gpa: address,
            insn_len: 1
        } if address == gpa
    ));

    restore.drv.set_page_access(DEFAULT_MEMORY_VIEW, gpa, Access::RWX).unwrap();
    restore
        .drv
        .toggle_intercept(0, InterceptType::Pagefault, false)
        .unwrap();
    restore
        .drv
        .reply_event(breakpoint, EventReplyType::EmulateInsn { data: original, view: KEEP_MEMORY_VIEW })
        .unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_emulated_int3_breakpoint",
    test_fn: vboxvmi_emulated_int3_breakpoint
});

/// A non-default view remaps RIP to an int3 in a Hidden page. The event reports that view,
/// an event reply switches back to Default, and physical reads stay in Default.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_memory_view_hidden_breakpoint(mut drv: Box<dyn Introspectable>) {
    assert_eq!(drv.get_vcpu_count().unwrap(), 1, "breakpoint test requires one vCPU");
    drv.pause().unwrap();
    drv.enable_memory_views(true).unwrap();
    let view = drv.create_memory_view().unwrap();
    let hidden = drv.allocate_hidden_page().unwrap();
    let regs = x86_registers(drv.as_ref(), 0);
    let gpa = translate(drv.as_ref(), regs.cr3, regs.rip).expect("RIP is not mapped");
    let page = gpa & !(PAGE - 1);
    let offset = (gpa - page) as usize;
    assert!(offset <= PAGE as usize - 16, "instruction crosses a page");
    let mut original_page = vec![0; PAGE as usize];
    drv.read_physical(page, &mut original_page, &mut 0).unwrap();
    let mut instruction = [0; 16];
    instruction.copy_from_slice(&original_page[offset..offset + 16]);
    assert_ne!(instruction[0], 0xcc, "RIP was already an int3");
    let mut hidden_page = original_page.clone();
    hidden_page[offset] = 0xcc;
    drv.write_physical(hidden * PAGE, &hidden_page).unwrap();
    drv.remap_memory_page(view, page / PAGE, hidden).unwrap();
    drv.toggle_intercept(0, InterceptType::Breakpoint, true)
        .unwrap();
    drv.switch_memory_view(view).unwrap();
    drv.resume().unwrap();

    let breakpoint = drv
        .listen(5000)
        .unwrap()
        .expect("remapped int3 did not raise a breakpoint");
    assert_eq!(breakpoint.view, view);
    assert!(matches!(
        breakpoint.kind,
        EventType::Breakpoint {
            gpa: address,
            insn_len: 1
        } if address == gpa
    ));
    let mut default_byte = [0];
    drv.read_physical(gpa, &mut default_byte, &mut 0).unwrap();
    assert_eq!(default_byte[0], instruction[0], "physical read used the Active view");
    // The Default view's page was never patched: switching to it and retrying the same RIP
    // (Continue without reinject) re-executes the original, untouched instruction.
    drv.reply_event(
        breakpoint,
        EventReplyType::Continue {
            reinject: false,
            view: DEFAULT_MEMORY_VIEW,
        },
    )
    .unwrap();
    drv.pause().unwrap(); // Liveness: the vCPU is running, not stuck re-faulting.
    drv.toggle_intercept(0, InterceptType::Breakpoint, false)
        .unwrap();
    drv.destroy_memory_view(view).unwrap();
    drv.free_hidden_page(hidden).unwrap();
    drv.enable_memory_views(false).unwrap();
    drv.resume().unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_memory_view_hidden_breakpoint",
    test_fn: vboxvmi_memory_view_hidden_breakpoint
});

/// Restricts the page of vCPU 0's RSP, the idle task's stack, to `allowed`, with the page-access
/// Intercept on vCPU 0. Returns RSP's gpa.
#[cfg(feature = "virtualbox-vmi")]
fn restrict_stack_page(drv: &mut dyn Introspectable, allowed: Access) -> u64 {
    drv.pause().unwrap();
    let regs = x86_registers(drv, 0);
    let gpa = translate(drv, regs.cr3, regs.rsp).expect("RSP is not mapped");
    drv.set_page_access(DEFAULT_MEMORY_VIEW, gpa, allowed).unwrap();
    drv.toggle_intercept(0, InterceptType::Pagefault, true)
        .unwrap();
    drv.resume().unwrap();
    gpa
}

/// The next page-access event of vCPU 0 on `page`, checking gva against gpa and that RIP is mapped.
#[cfg(feature = "virtualbox-vmi")]
fn data_event(drv: &mut dyn Introspectable, page: u64) -> (Event, Access, u64, X86Registers) {
    let event = drv.listen(5000).unwrap().expect("no page-access event");
    assert_eq!(
        event.vcpu, 0,
        "page-access event from a vCPU without the Intercept"
    );
    let (gva, gpa, access) = page_access_event(&event);
    let regs = x86_registers(drv, 0);
    assert_eq!(gpa / PAGE, page / PAGE, "event outside the restricted page");
    assert!(
        gva == 0 || translate(drv, regs.cr3, gva) == Some(gpa),
        "gva {:#x} is not gpa {:#x}",
        gva,
        gpa
    );
    assert!(
        translate(drv, regs.cr3, regs.rip).is_some(),
        "RIP is not mapped"
    );
    (event, access, gpa, regs)
}

/// Write restriction on the idle stack: write events, a plain retry faulting again at the same
/// RIP, reinjection refused, no event once lifted.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_page_access_write(mut drv: Box<dyn Introspectable>) {
    let page = restrict_stack_page(drv.as_mut(), Access::RX);
    let (event, access, gpa, regs) = data_event(drv.as_mut(), page);
    assert_eq!(access, Access::W);
    assert!(drv
        .reply_event(
            event.clone(),
            EventReplyType::Continue {
                reinject: true,
                view: KEEP_MEMORY_VIEW,
            },
        )
        .is_err());
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let (event, access, retried, again) = data_event(drv.as_mut(), page);
    assert_eq!(
        (access, retried, again.rip),
        (Access::W, gpa, regs.rip),
        "retry did not fault again"
    );
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv.as_mut(), &mut || (), "page-access event after lifting");
    drv.toggle_intercept(0, InterceptType::Pagefault, false)
        .unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_page_access_write",
    test_fn: vboxvmi_page_access_write
});

/// The register `POP` (`58+r`, `41 58+r`) writes, `None` for any other instruction and for
/// `POP RSP`.
#[cfg(feature = "virtualbox-vmi")]
fn pop_register<'a>(code: &[u8], regs: &'a mut X86Registers) -> Option<&'a mut u64> {
    let (high, op) = match code {
        [0x41, op @ 0x58..=0x5f, ..] => (8, op),
        [op @ 0x58..=0x5f, ..] => (0, op),
        _ => return None,
    };
    Some(match high + (op & 7) {
        0 => &mut regs.rax,
        1 => &mut regs.rcx,
        2 => &mut regs.rdx,
        3 => &mut regs.rbx,
        5 => &mut regs.rbp,
        6 => &mut regs.rsi,
        7 => &mut regs.rdi,
        8 => &mut regs.r8,
        9 => &mut regs.r9,
        10 => &mut regs.r10,
        11 => &mut regs.r11,
        12 => &mut regs.r12,
        13 => &mut regs.r13,
        14 => &mut regs.r14,
        15 => &mut regs.r15,
        _ => return None,
    })
}

/// No access to the idle stack: read events; `Emulate` lets the other accesses through until a
/// `POP` reads the page, whose `EmulateReadData` must land in its register (then restored to the
/// stack's value, the guest must not run with it).
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_page_access_emulate_read_data(mut drv: Box<dyn Introspectable>) {
    const DATA: u64 = 0x1122_3344_5566_7788;
    let page = restrict_stack_page(drv.as_mut(), Access::NIL);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (event, real, regs) = loop {
        assert!(
            Instant::now() < deadline,
            "no POP reading the restricted stack"
        );
        let (event, access, gpa, mut regs) = data_event(drv.as_mut(), page);
        let code = code_at_rip(drv.as_ref(), &regs, 2);
        if access == Access::R && pop_register(&code, &mut regs).is_some() {
            let mut real = [0; 8];
            drv.read_physical(gpa, &mut real, &mut 0).unwrap();
            break (event, u64::from_le_bytes(real), regs);
        }
        drv.reply_event(event, EventReplyType::Emulate { view: KEEP_MEMORY_VIEW }).unwrap();
    };
    assert!(drv
        .reply_event(
            event.clone(),
            EventReplyType::EmulateInsn {
                data: [0x90; 16],
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    let mut data = [0; 256];
    data[..8].copy_from_slice(&DATA.to_le_bytes());
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    drv.reply_event(
        event,
        EventReplyType::EmulateReadData {
            size: 8,
            data,
            view: KEEP_MEMORY_VIEW,
        },
    )
        .unwrap();
    let step = drv
        .listen(5000)
        .unwrap()
        .expect("no step after EmulateReadData");
    assert!(
        matches!(step.kind, EventType::Singlestep { .. }),
        "{:?}",
        step.kind
    );
    let code = code_at_rip(drv.as_ref(), &regs, 2);
    let mut after = x86_registers(drv.as_ref(), 0);
    let register = pop_register(&code, &mut after).unwrap();
    assert_eq!(*register, DATA, "POP did not read the supplied data");
    *register = real;
    drv.write_registers(0, Registers::X86(after)).unwrap();
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();
    drv.toggle_intercept(0, InterceptType::Pagefault, false)
        .unwrap();
    drv.reply_event(step, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_page_access_emulate_read_data",
    test_fn: vboxvmi_page_access_emulate_read_data
});

/// With single step also set, the page-access event of the first instruction comes first, then,
/// once the restriction is lifted, the step past it.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_page_access_before_singlestep(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    drv.toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    let page = restrict_rip_page(drv.as_mut(), Access::RW);
    let (event, rip) = execute_event(drv.as_mut(), page);
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let step = drv
        .listen(5000)
        .unwrap()
        .expect("no step after the page-access event");
    assert!(
        matches!(step.kind, EventType::Singlestep { .. }),
        "{:?}",
        step.kind
    );
    assert_ne!(
        x86_registers(drv.as_ref(), 0).rip,
        rip,
        "step before the instruction ran"
    );
    drv.toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    drv.toggle_intercept(0, InterceptType::Pagefault, false)
        .unwrap();
    drv.reply_event(step, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv.as_mut(), &mut || (), "event after clear");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_page_access_before_singlestep",
    test_fn: vboxvmi_page_access_before_singlestep
});

/// `EmulateInsn` at a breakpoint runs supplied safe bytes while the int3 stays in guest
/// memory; page-access emulation replies don't fit a breakpoint.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_emulate_insn(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    let before = x86_registers(drv.as_ref(), 0);
    let gpa = translate(drv.as_ref(), before.cr3, before.rip).expect("RIP is not mapped");
    let mut data = [0; 16];
    data.copy_from_slice(&code_at_rip(drv.as_ref(), &before, 16));
    assert_ne!(data[0], 0xcc, "RIP was already an int3");
    let mut restore = BreakpointRestore {
        drv,
        gpa,
        original: data[0],
        patched: true,
        intercepted: true,
        paused: true,
    };
    restore.drv.write_physical(gpa, &[0xcc]).unwrap();
    restore
        .drv
        .toggle_intercept(0, InterceptType::Breakpoint, true)
        .unwrap();
    restore.drv.resume().unwrap();
    restore.paused = false;
    let event = restore.drv.listen(5000).unwrap().expect("no int3 at RIP");
    assert!(matches!(event.kind, EventType::Breakpoint { gpa: addr, .. } if addr == gpa));
    assert!(restore
        .drv
        .reply_event(
            event.clone(),
            EventReplyType::Emulate {
                view: KEEP_MEMORY_VIEW,
            },
        )
        .is_err());
    assert!(restore
        .drv
        .reply_event(
            event.clone(),
            EventReplyType::EmulateReadData {
                size: 1,
                data: [0; 256],
                view: KEEP_MEMORY_VIEW,
            }
        )
        .is_err());
    let replacement = [
        0xb8, 0x34, 0x12, 0, 0, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    ];
    restore
        .drv
        .toggle_intercept(0, InterceptType::Singlestep, true)
        .unwrap();
    restore
        .drv
        .reply_event(event, EventReplyType::EmulateInsn { data: replacement, view: KEEP_MEMORY_VIEW })
        .unwrap();
    let step = restore
        .drv
        .listen(5000)
        .unwrap()
        .expect("no step after EmulateInsn");
    assert!(
        matches!(step.kind, EventType::Singlestep { .. }),
        "{:?}",
        step.kind
    );
    let after = x86_registers(restore.drv.as_ref(), 0);
    assert_eq!(after.rax as u32, 0x1234, "supplied instruction did not run");
    assert_eq!(after.rip, before.rip + 5, "instruction did not advance RIP");
    let mut byte = [0];
    restore.drv.read_physical(gpa, &mut byte, &mut 0).unwrap();
    assert_eq!(byte[0], 0xcc, "int3 left guest memory");
    restore.drv.pause().unwrap();
    restore.paused = true;
    restore.drv.write_physical(gpa, &data[..1]).unwrap();
    restore.patched = false;
    restore
        .drv
        .toggle_intercept(0, InterceptType::Singlestep, false)
        .unwrap();
    restore
        .drv
        .reply_event(step, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_emulate_insn",
    test_fn: vboxvmi_emulate_insn
});

/// Page access restrictions and the VirtualBox debugger exclude each other like Intercepts;
/// lifting a restriction works while it is attached.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_page_access_debugger_exclusion(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    let regs = x86_registers(drv.as_ref(), 0);
    let page = translate(drv.as_ref(), regs.cr3, regs.rsp).expect("RSP is not mapped");
    drv.resume().unwrap();
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RX).unwrap();
    let (_, output) = debugger_console();
    assert!(
        output.contains("VERR_DBGF_VMI_INTERCEPTS_ACTIVE"),
        "debugger attached with a page access restriction: {:?}",
        output
    );
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();

    let (console, output) = debugger_console();
    assert!(
        output.ends_with("VBoxDbg> "),
        "debugger did not attach: {:?}",
        output
    );
    let error = drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RX)
        .expect_err("restriction set with the debugger attached");
    assert!(error.to_string().contains("debugger"), "{}", error);
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();

    drop(console);
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Err(e) = drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RX) {
        assert!(Instant::now() < deadline, "debugger still attached: {}", e);
        sleep(Duration::from_millis(100));
    }
    drv.set_page_access(DEFAULT_MEMORY_VIEW, page, Access::RWX).unwrap();
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_page_access_debugger_exclusion",
    test_fn: vboxvmi_page_access_debugger_exclusion
});

/// x2APIC TMICT (LAPIC timer initial count), which the idle test guest writes about 900 times a
/// second; it also writes the x2APIC EOI (0x80b) as often, without the Intercept.
#[cfg(feature = "virtualbox-vmi")]
const TEST_MSR: u32 = 0x838;

/// Write events for `TEST_MSR` with its Intercept on vCPU 0 only: the right MSR index on the
/// right vCPU only (the guest writes other MSRs too), reported before the write with RIP at a
/// `WRMSR` (ECX the index, EDX:EAX the new value). Each old value is the previous event's new one
/// (TMICT reads back what was written), so the write proceeded after the reply. Reinjection is
/// refused and leaves the vCPU parked; none after clearing the Intercept from within event
/// handling.
#[cfg(feature = "virtualbox-vmi")]
fn vboxvmi_msr_write(mut drv: Box<dyn Introspectable>) {
    let intercept = InterceptType::Msr(TEST_MSR);
    drv.toggle_intercept(0, intercept, true).unwrap();
    let mut previous_new = None;
    for _ in 0..8 {
        let event = next_event(drv.as_mut(), &mut || {});
        assert_eq!(event.vcpu, 0, "MSR event from a vCPU without the Intercept");
        let (old, new) = match event.kind {
            EventType::Msr {
                msr_type,
                value,
                old,
            } => {
                assert_eq!(msr_type, TEST_MSR, "event for an MSR without the Intercept");
                (old, value)
            }
            ref other => panic!("not an MSR write event: {:?}", other),
        };
        if let Some(previous) = previous_new {
            assert_eq!(old, previous, "MSR is not the previous write's new value");
        }
        previous_new = Some(new);
        let regs = x86_registers(drv.as_ref(), 0);
        assert_eq!(regs.rcx as u32, TEST_MSR, "ECX is not the MSR index");
        assert_eq!(
            new,
            (regs.rdx << 32) | (regs.rax & 0xffff_ffff),
            "new value is not EDX:EAX"
        );
        let code = code_at_rip(drv.as_ref(), &regs, 4);
        assert_eq!(
            code[..2],
            [0x0f, 0x30],
            "RIP {:#x} is not a WRMSR",
            regs.rip
        );
        let reinjected = drv.reply_event(
            Event { vcpu: 0, view: DEFAULT_MEMORY_VIEW, kind: EventType::Msr {
                msr_type: TEST_MSR,
                value: new,
                old,
            } },
            EventReplyType::Continue { reinject: true, view: KEEP_MEMORY_VIEW },
        );
        assert!(reinjected.is_err(), "MSR event reinjected");
        assert_eq!(x86_registers(drv.as_ref(), 0).rip, regs.rip);
        drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
            .unwrap();
    }
    let event = next_event(drv.as_mut(), &mut || {});
    drv.toggle_intercept(0, intercept, false).unwrap();
    drv.reply_event(event, EventReplyType::Continue { reinject: false, view: KEEP_MEMORY_VIEW })
        .unwrap();
    assert_no_event(drv.as_mut(), &mut || {}, "MSR event after clear");
}

#[cfg(feature = "virtualbox-vmi")]
inventory::submit!(IntegrationTest {
    name: "vboxvmi_events_msr_write",
    test_fn: vboxvmi_msr_write
});

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

/// Both VirtualBox access methods attached to one VM, compared without a core dump.
#[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
mod cross_method {
    use super::*;
    use crate::common::filter::CROSS_METHOD_TEST;
    use microvmi::api::params::{CommonInitParams, DriverInitParams, VBoxInitParams};
    use microvmi::api::DriverType;

    fn read_exact(drv: &dyn Introspectable, method: &str, addr: u64, buf: &mut [u8]) {
        let mut bytes_read = 0;
        drv.read_physical(addr, buf, &mut bytes_read)
            .unwrap_or_else(|error| {
                panic!(
                    "{} failed to read RAM at {:#x} after {} bytes: {}",
                    method, addr, bytes_read, error
                )
            });
        assert_eq!(
            bytes_read,
            buf.len() as u64,
            "{} short read at {:#x}",
            method,
            addr
        );
    }

    fn read_page(drv: &dyn Introspectable, method: &str, addr: u64) -> Vec<u8> {
        let mut page = vec![0; PAGE as usize];
        read_exact(drv, method, addr, &mut page);
        page
    }

    fn assert_page(drv: &dyn Introspectable, method: &str, addr: u64, expected: &[u8]) {
        let actual = read_page(drv, method, addr);
        if let Some(offset) = actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected)
        {
            panic!(
                "{} RAM differs at {:#x}: {:#04x} != {:#04x}",
                method,
                addr + offset as u64,
                actual[offset],
                expected[offset]
            );
        }
    }

    fn assert_registers(
        drv: &dyn Introspectable,
        method: &str,
        vcpu: u16,
        expected: &X86Registers,
    ) {
        assert_eq!(
            x86_registers(drv, vcpu),
            *expected,
            "{} registers differ on vCPU {}",
            method,
            vcpu
        );
    }

    fn pattern(addr: u64, salt: u64) -> Vec<u8> {
        let mut state = (addr ^ salt) | 1;
        (0..PAGE)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect()
    }

    /// Restores through both methods before `ResumeVm` resumes, including during unwinding. The
    /// outer VirtualBox context then powers off and restores the base snapshot.
    struct CrossMethodRestore<'a> {
        machine_debugger: &'a dyn Introspectable,
        vboxvmi: &'a dyn Introspectable,
        pages: Vec<(u64, Vec<u8>)>,
        vcpu: u16,
        registers: X86Registers,
        armed: bool,
    }

    impl CrossMethodRestore<'_> {
        fn restore_with(&self, drv: &dyn Introspectable, method: &str) {
            for (addr, original) in &self.pages {
                if let Err(error) = drv.write_physical(*addr, original) {
                    eprintln!("{} failed to restore RAM at {:#x}: {}", method, addr, error);
                }
            }
            if let Err(error) =
                drv.write_registers(self.vcpu, Registers::X86(self.registers.clone()))
            {
                eprintln!(
                    "{} failed to restore vCPU {} registers: {}",
                    method, self.vcpu, error
                );
            }
        }
    }

    impl Drop for CrossMethodRestore<'_> {
        fn drop(&mut self) {
            if self.armed {
                let _ = try_vboxmanage(&["controlvm", &vm_name(), "pause"]);
                self.restore_with(self.machine_debugger, "MachineDebugger");
                self.restore_with(self.vboxvmi, "VBoxVMI");
            }
        }
    }

    fn init_machine_debugger() -> Box<dyn Introspectable> {
        microvmi::init(
            Some(DriverType::VirtualBox),
            Some(DriverInitParams {
                common: Some(CommonInitParams { vm_name: vm_name() }),
                virtualbox: Some(VBoxInitParams::MachineDebugger),
                ..Default::default()
            }),
        )
        .expect("failed to attach MachineDebugger")
    }

    /// On one paused VM, compare every register of every vCPU and every byte of all PGM RAM ranges
    /// directly between the methods. Then write safe RAM pages and SYSENTER registers in both
    /// directions, with independent readback and exact restoration through both methods.
    fn consistency(vboxvmi: Box<dyn Introspectable>) {
        let filter = env::var("MICROVMI_TEST_FILTER").ok();
        if !matches!(env::var("VBOX_TEST_CROSS_METHOD").as_deref(), Ok("1")) {
            if filter.is_none() {
                println!(
                    "SKIPPED: set VBOX_TEST_CROSS_METHOD=1 and \
                     MICROVMI_TEST_FILTER={}",
                    CROSS_METHOD_TEST
                );
                return;
            }
            panic!("cross-method test requires VBOX_TEST_CROSS_METHOD=1");
        }
        assert_eq!(
            filter.as_deref(),
            Some(CROSS_METHOD_TEST),
            "cross-method test requires the exact MICROVMI_TEST_FILTER={}",
            CROSS_METHOD_TEST
        );
        assert!(
            matches!(env::var("VBOX_TEST_METHOD").as_deref(), Ok("vboxvmi")),
            "cross-method test requires VBOX_TEST_METHOD=vboxvmi"
        );
        let mut machine_debugger = init_machine_debugger();
        let _resume = ResumeVm;
        machine_debugger.pause().unwrap();
        assert_eq!(vm_info("VMState"), "paused");

        let md_count = machine_debugger.get_vcpu_count().unwrap();
        let vmi_count = vboxvmi.get_vcpu_count().unwrap();
        assert_eq!(md_count, vmi_count, "vCPU counts differ");
        assert!(
            md_count >= 2,
            "cross-method coverage requires VBOX_TEST_CPUS=2 or more"
        );
        for vcpu in 0..md_count {
            let md = x86_registers(machine_debugger.as_ref(), vcpu);
            let vmi = x86_registers(vboxvmi.as_ref(), vcpu);
            assert_eq!(md, vmi, "complete registers differ on vCPU {}", vcpu);
        }

        let ranges = ram_ranges();
        let ranges_end = ranges.iter().map(|range| range.end).max().unwrap();
        let md_max = machine_debugger.get_max_physical_addr().unwrap();
        let vmi_max = vboxvmi.get_max_physical_addr().unwrap();
        assert_eq!(md_max, vmi_max, "maximum physical addresses differ");
        assert_eq!(md_max, ranges_end, "driver maximum does not cover PGM RAM");

        // The same safe locations as the core-based write test. Record the nearest all-zero page
        // below each target while the complete direct comparison already has every RAM page.
        let top = ranges_end;
        let top_below_4g = ranges
            .iter()
            .map(|range| range.end)
            .filter(|&end| end <= FOUR_GIB)
            .max()
            .unwrap();
        let write_targets = [0x10000, top / 2, top_below_4g, top];
        let target_ranges: Vec<(u64, u64)> = write_targets
            .iter()
            .map(|&target| {
                let range = ranges
                    .iter()
                    .find(|range| range.start < target && target <= range.end)
                    .unwrap_or_else(|| panic!("{:#x} is not in a RAM range", target));
                (target & !(PAGE - 1), range.start.max(PAGE))
            })
            .collect();
        let mut write_pages = vec![None; write_targets.len()];

        let mut md = vec![0; CHUNK as usize];
        let mut vmi = vec![0; CHUNK as usize];
        let (mut compared, mut compared_above_4g) = (0u64, 0u64);
        for range in &ranges {
            let mut addr = range.start;
            while addr < range.end {
                let len = CHUNK.min(range.end - addr) as usize;
                read_exact(
                    machine_debugger.as_ref(),
                    "MachineDebugger",
                    addr,
                    &mut md[..len],
                );
                read_exact(vboxvmi.as_ref(), "VBoxVMI", addr, &mut vmi[..len]);
                if let Some(offset) = md[..len]
                    .iter()
                    .zip(&vmi[..len])
                    .position(|(md, vmi)| md != vmi)
                {
                    panic!(
                        "methods differ at guest-physical {:#x}: {:#04x} != {:#04x}",
                        addr + offset as u64,
                        md[offset],
                        vmi[offset]
                    );
                }
                let (pages, remainder) = md[..len].as_chunks::<{ PAGE as usize }>();
                debug_assert!(remainder.is_empty());
                for (offset, page) in pages.iter().enumerate() {
                    if page.iter().any(|&byte| byte != 0) {
                        continue;
                    }
                    let page = addr + offset as u64 * PAGE;
                    for ((target, first), selected) in target_ranges.iter().zip(&mut write_pages) {
                        if *first <= page && page < *target {
                            *selected = Some(page);
                        }
                    }
                }
                compared += len as u64;
                compared_above_4g += (addr + len as u64).saturating_sub(addr.max(FOUR_GIB));
                addr += len as u64;
            }
        }
        let ram_bytes: u64 = ranges.iter().map(|range| range.end - range.start).sum();
        let ram_above_4g: u64 = ranges
            .iter()
            .map(|range| range.end.saturating_sub(range.start.max(FOUR_GIB)))
            .sum();
        assert_eq!(compared, ram_bytes);
        assert_eq!(compared_above_4g, ram_above_4g);
        if vm_info("memory").parse::<u64>().unwrap() > 4096 {
            assert!(
                md_max > FOUR_GIB,
                "maximum physical address is not above 4 GiB"
            );
            assert!(
                compared_above_4g > 0,
                "no RAM bytes above 4 GiB were compared"
            );
        }
        println!(
            "cross-method compared {} RAM bytes ({} above 4 GiB), max physical address {:#x}, {} vCPUs",
            compared, compared_above_4g, md_max, md_count
        );
        print!("PGM RAM ranges");
        for range in &ranges {
            print!(" {:#x}..{:#x}", range.start, range.end);
        }
        println!();

        let mut page_addrs: Vec<u64> = write_pages
            .into_iter()
            .zip(write_targets)
            .map(|(page, target)| {
                page.unwrap_or_else(|| panic!("no zero page below {:#x}", target))
            })
            .collect();
        page_addrs.sort_unstable();
        page_addrs.dedup();
        if ram_above_4g > 0 {
            assert!(
                page_addrs.iter().any(|&addr| addr >= FOUR_GIB),
                "no write page above 4 GiB"
            );
        }
        let pages: Vec<(u64, Vec<u8>)> = page_addrs
            .iter()
            .map(|&addr| {
                (
                    addr,
                    read_page(machine_debugger.as_ref(), "MachineDebugger", addr),
                )
            })
            .collect();
        for (addr, original) in &pages {
            assert_page(vboxvmi.as_ref(), "VBoxVMI", *addr, original);
        }
        let vcpu = md_count - 1;
        let registers = x86_registers(machine_debugger.as_ref(), vcpu);
        assert_registers(vboxvmi.as_ref(), "VBoxVMI", vcpu, &registers);
        let mut restore = CrossMethodRestore {
            machine_debugger: machine_debugger.as_ref(),
            vboxvmi: vboxvmi.as_ref(),
            pages,
            vcpu,
            registers,
            armed: true,
        };

        for (addr, original) in &restore.pages {
            let changed = pattern(*addr, 0x004d_442d_3e56_4d49);
            machine_debugger.write_physical(*addr, &changed).unwrap();
            assert_page(vboxvmi.as_ref(), "VBoxVMI", *addr, &changed);
            vboxvmi.write_physical(*addr, original).unwrap();
            assert_page(
                machine_debugger.as_ref(),
                "MachineDebugger",
                *addr,
                original,
            );
            assert_page(vboxvmi.as_ref(), "VBoxVMI", *addr, original);

            let changed = pattern(*addr, 0x0056_4d49_2d3e_4d44);
            vboxvmi.write_physical(*addr, &changed).unwrap();
            assert_page(
                machine_debugger.as_ref(),
                "MachineDebugger",
                *addr,
                &changed,
            );
        }

        let mut changed = restore.registers.clone();
        changed.sysenter_esp ^= 8;
        changed.sysenter_eip ^= 8;
        machine_debugger
            .write_registers(vcpu, Registers::X86(changed.clone()))
            .unwrap();
        assert_registers(vboxvmi.as_ref(), "VBoxVMI", vcpu, &changed);
        vboxvmi
            .write_registers(vcpu, Registers::X86(restore.registers.clone()))
            .unwrap();
        assert_registers(
            machine_debugger.as_ref(),
            "MachineDebugger",
            vcpu,
            &restore.registers,
        );
        assert_registers(vboxvmi.as_ref(), "VBoxVMI", vcpu, &restore.registers);

        changed = restore.registers.clone();
        changed.sysenter_esp ^= 16;
        changed.sysenter_eip ^= 16;
        vboxvmi
            .write_registers(vcpu, Registers::X86(changed.clone()))
            .unwrap();
        assert_registers(machine_debugger.as_ref(), "MachineDebugger", vcpu, &changed);

        // Final restoration is deliberately performed through both methods and verified through
        // both, so either method can recover the state left by the other.
        for (addr, original) in &restore.pages {
            vboxvmi.write_physical(*addr, original).unwrap();
            assert_page(
                machine_debugger.as_ref(),
                "MachineDebugger",
                *addr,
                original,
            );
            assert_page(vboxvmi.as_ref(), "VBoxVMI", *addr, original);
            machine_debugger.write_physical(*addr, original).unwrap();
            assert_page(
                machine_debugger.as_ref(),
                "MachineDebugger",
                *addr,
                original,
            );
            assert_page(vboxvmi.as_ref(), "VBoxVMI", *addr, original);
        }
        vboxvmi
            .write_registers(vcpu, Registers::X86(restore.registers.clone()))
            .unwrap();
        assert_registers(
            machine_debugger.as_ref(),
            "MachineDebugger",
            vcpu,
            &restore.registers,
        );
        assert_registers(vboxvmi.as_ref(), "VBoxVMI", vcpu, &restore.registers);
        machine_debugger
            .write_registers(vcpu, Registers::X86(restore.registers.clone()))
            .unwrap();
        assert_registers(
            machine_debugger.as_ref(),
            "MachineDebugger",
            vcpu,
            &restore.registers,
        );
        assert_registers(vboxvmi.as_ref(), "VBoxVMI", vcpu, &restore.registers);
        restore.armed = false;
        let pages: Vec<String> = page_addrs
            .iter()
            .map(|addr| format!("{:#x}", addr))
            .collect();
        println!(
            "cross-method writes restored on vCPU {} and RAM pages {}",
            vcpu,
            pages.join(", ")
        );
        drop(restore);

        machine_debugger.resume().unwrap();
        assert_eq!(vm_info("VMState"), "running");
    }

    inventory::submit!(IntegrationTest {
        name: CROSS_METHOD_TEST,
        test_fn: consistency
    });
}
