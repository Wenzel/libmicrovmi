use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{App, Arg, ArgMatches};

use microvmi::api::events::{Event, EventReplyType, EventType, InterceptType};
use microvmi::api::params::DriverInitParams;
use microvmi::api::registers::Registers;
use microvmi::api::{
    Access, Introspectable, DEFAULT_MEMORY_VIEW, KEEP_MEMORY_VIEW, PAGE_SIZE, RESTORE_PAGE,
};
use utilities::Clappable;

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;
const PAGE: u64 = PAGE_SIZE as u64;
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, PartialEq)]
enum Method {
    PageAccess,
    MemoryView,
}

#[derive(Clone, Copy)]
enum Pending {
    None,
    PageStep { write: bool },
    ViewStep { write: bool },
}

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Plants a symbol-free stealth breakpoint at MSR_LSTAR or a guest-physical address")
        .arg(
            Arg::with_name("method")
                .long("method")
                .takes_value(true)
                .possible_values(&["page-access", "memory-view"])
                .default_value("page-access")
                .help("Stealth breakpoint method"),
        )
        .arg(Arg::with_name("gpa").long("gpa").takes_value(true).help(
            "Target guest-physical address, decimal or 0x-prefixed hex (default: vCPU 0 MSR_LSTAR)",
        ))
        .arg(
            Arg::with_name("count")
                .long("count")
                .short("n")
                .takes_value(true)
                .default_value("0")
                .help("Stop after this many hits; 0 waits for Ctrl-C"),
        )
        .args(DriverInitParams::to_clap_args().as_ref())
        .get_matches()
}

fn number(value: &str) -> Result<u64> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    };
    parsed.map_err(|error| IoError::new(ErrorKind::InvalidInput, error).into())
}

fn x86_registers(
    drv: &dyn Introspectable,
    vcpu: u16,
) -> Result<microvmi::api::registers::X86Registers> {
    match drv.read_registers(vcpu)? {
        Registers::X86(regs) => Ok(regs),
    }
}

fn translate(drv: &dyn Introspectable, cr3: u64, cr4: u64, vaddr: u64) -> Result<u64> {
    let shifts: &[u32] = if cr4 & (1 << 12) != 0 {
        &[48, 39, 30, 21, 12]
    } else {
        &[39, 30, 21, 12]
    };
    let mut table = cr3 & ADDR_MASK;
    for &shift in shifts {
        let mut entry = [0; 8];
        let mut read = 0;
        drv.read_physical(
            table + ((vaddr >> shift) & 0x1ff) * 8,
            &mut entry,
            &mut read,
        )?;
        if read != 8 {
            return Err(IoError::new(ErrorKind::UnexpectedEof, "short page-table read").into());
        }
        let entry = u64::from_le_bytes(entry);
        if entry & 1 == 0 {
            return Err(
                IoError::new(ErrorKind::NotFound, "target virtual address is not mapped").into(),
            );
        }
        let large = shift != 48 && shift != 39 && shift != 12 && entry & 0x80 != 0;
        if large || shift == 12 {
            let offset = (1u64 << shift) - 1;
            return Ok((entry & ADDR_MASK & !offset) | (vaddr & offset));
        }
        table = entry & ADDR_MASK;
    }
    unreachable!()
}

struct Session {
    drv: Box<dyn Introspectable>,
    method: Method,
    target: u64,
    page: u64,
    offset: usize,
    logical: Vec<u8>,
    instruction: [u8; 16],
    view: Option<u16>,
    hidden: Option<u64>,
    pending: Vec<Pending>,
    parked: Vec<Event>,
    deferred: Vec<Event>,
    paused: bool,
    active: bool,
}

impl Session {
    fn new(
        mut drv: Box<dyn Introspectable>,
        method: Method,
        explicit_gpa: Option<u64>,
    ) -> Result<Self> {
        drv.pause()?;
        let prepared = (|| {
            let regs = x86_registers(drv.as_ref(), 0)?;
            let (target, description) = match explicit_gpa {
                Some(gpa) => (gpa, format!("GPA {:#x}", gpa)),
                None => {
                    let gpa = translate(drv.as_ref(), regs.cr3, regs.cr4, regs.msr_lstar)?;
                    (
                        gpa,
                        format!("MSR_LSTAR {:#x} -> GPA {:#x}", regs.msr_lstar, gpa),
                    )
                }
            };
            let page = target & !(PAGE - 1);
            let offset = (target - page) as usize;
            if offset + 16 > PAGE as usize {
                return Err(IoError::new(
                    ErrorKind::InvalidInput,
                    "the target's first 16 bytes must fit in one physical page",
                )
                .into());
            }
            let mut logical = vec![0; PAGE as usize];
            let mut read = 0;
            drv.read_physical(page, &mut logical, &mut read)?;
            if read != PAGE {
                return Err(
                    IoError::new(ErrorKind::UnexpectedEof, "short target-page read").into(),
                );
            }
            if logical[offset] == 0xcc {
                return Err(
                    IoError::new(ErrorKind::InvalidInput, "target already contains int3").into(),
                );
            }
            let mut instruction = [0; 16];
            instruction.copy_from_slice(&logical[offset..offset + 16]);
            let count = drv.get_vcpu_count()?;
            Ok((
                target,
                description,
                page,
                offset,
                logical,
                instruction,
                count,
            ))
        })();
        let (target, description, page, offset, logical, instruction, count) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = drv.resume();
                return Err(error);
            }
        };
        println!(
            "Target: {} ({})",
            description,
            if method == Method::PageAccess {
                "page-access"
            } else {
                "memory-view"
            }
        );
        Ok(Session {
            drv,
            method,
            target,
            page,
            offset,
            logical,
            instruction,
            view: None,
            hidden: None,
            pending: vec![Pending::None; count as usize],
            parked: Vec::new(),
            deferred: Vec::new(),
            paused: true,
            active: false,
        })
    }

    fn toggle_all(&mut self, intercept: InterceptType, enabled: bool) -> Result<()> {
        for vcpu in 0..self.pending.len() {
            self.drv.toggle_intercept(vcpu as u16, intercept, enabled)?;
        }
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        self.active = true;
        self.toggle_all(InterceptType::Pagefault, true)?;
        self.toggle_all(InterceptType::Breakpoint, true)?;
        match self.method {
            Method::PageAccess => {
                self.drv.write_physical(self.target, &[0xcc])?;
                self.drv
                    .set_page_access(DEFAULT_MEMORY_VIEW, self.page, Access::NIL)?;
            }
            Method::MemoryView => {
                self.drv.enable_memory_views(true)?;
                let view = self.drv.create_memory_view()?;
                self.view = Some(view);
                let hidden = self.drv.allocate_hidden_page()?;
                self.hidden = Some(hidden);
                let mut copy = self.logical.clone();
                copy[self.offset] = 0xcc;
                self.drv.write_physical(hidden * PAGE, &copy)?;
                self.drv.remap_memory_page(view, self.page / PAGE, hidden)?;
                self.drv.set_page_access(view, self.page, Access::X)?;
                self.drv.switch_memory_view(view)?;
            }
        }
        self.drv.resume()?;
        self.paused = false;
        Ok(())
    }

    fn reply(&mut self, event: Event, reply: EventReplyType) -> Result<()> {
        let parked = event.clone();
        match self.drv.reply_event(event, reply) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.parked.push(parked);
                Err(error)
            }
        }
    }

    fn handle_page_access(&mut self, event: Event, access: Access) -> Result<()> {
        match access {
            Access::R | Access::W => {
                self.drv
                    .write_physical(self.target, &[self.logical[self.offset]])?;
                self.drv
                    .toggle_intercept(event.vcpu, InterceptType::Singlestep, true)?;
                self.pending[event.vcpu as usize] = Pending::PageStep {
                    write: access == Access::W,
                };
                let reply = if access == Access::W {
                    EventReplyType::Emulate {
                        view: KEEP_MEMORY_VIEW,
                    }
                } else {
                    EventReplyType::EmulateNoWrite {
                        view: KEEP_MEMORY_VIEW,
                    }
                };
                self.reply(event, reply)
            }
            Access::X => {
                let retry = event.clone();
                if self
                    .drv
                    .reply_event(
                        event,
                        EventReplyType::Emulate {
                            view: KEEP_MEMORY_VIEW,
                        },
                    )
                    .is_ok()
                {
                    Ok(())
                } else {
                    self.reply(
                        retry,
                        EventReplyType::UnrestrictedStep {
                            view: KEEP_MEMORY_VIEW,
                        },
                    )
                }
            }
            _ => Err(IoError::new(ErrorKind::InvalidData, "page event has invalid access").into()),
        }
    }

    fn handle_view_access(&mut self, event: Event, access: Access) -> Result<()> {
        if access != Access::R && access != Access::W {
            return Err(
                IoError::new(ErrorKind::InvalidData, "execute fault in execute view").into(),
            );
        }
        self.drv
            .toggle_intercept(event.vcpu, InterceptType::Singlestep, true)?;
        self.pending[event.vcpu as usize] = Pending::ViewStep {
            write: access == Access::W,
        };
        self.reply(
            event,
            EventReplyType::Continue {
                reinject: false,
                view: DEFAULT_MEMORY_VIEW,
            },
        )
    }

    fn handle_step(&mut self, event: Event) -> Result<()> {
        let pending = self.pending[event.vcpu as usize];
        self.drv
            .toggle_intercept(event.vcpu, InterceptType::Singlestep, false)?;
        self.pending[event.vcpu as usize] = Pending::None;
        match pending {
            Pending::PageStep { write } => {
                if write {
                    self.drv
                        .read_physical(self.page, &mut self.logical, &mut 0)?;
                    self.instruction
                        .copy_from_slice(&self.logical[self.offset..self.offset + 16]);
                }
                self.drv.write_physical(self.target, &[0xcc])?;
                self.reply(
                    event,
                    EventReplyType::Continue {
                        reinject: false,
                        view: KEEP_MEMORY_VIEW,
                    },
                )
            }
            Pending::ViewStep { write } => {
                if write {
                    self.drv
                        .read_physical(self.page, &mut self.logical, &mut 0)?;
                    self.instruction
                        .copy_from_slice(&self.logical[self.offset..self.offset + 16]);
                    let mut copy = self.logical.clone();
                    copy[self.offset] = 0xcc;
                    self.drv
                        .write_physical(self.hidden.unwrap() * PAGE, &copy)?;
                }
                self.reply(
                    event,
                    EventReplyType::Continue {
                        reinject: false,
                        view: self.view.unwrap(),
                    },
                )
            }
            Pending::None => {
                Err(IoError::new(ErrorKind::InvalidData, "unexpected single-step event").into())
            }
        }
    }

    fn next_event(&mut self, timeout: u32) -> Result<Option<Event>> {
        let page_step = self
            .pending
            .iter()
            .any(|pending| matches!(pending, Pending::PageStep { .. }));
        if !page_step {
            if let Some(event) = self.deferred.pop() {
                return Ok(Some(event));
            }
        }
        self.drv.listen(timeout)
    }

    fn handle(&mut self, event: Event, hits: &mut [u64]) -> Result<bool> {
        let page_step = self
            .pending
            .iter()
            .any(|pending| matches!(pending, Pending::PageStep { .. }));
        let expected_step = matches!(self.pending[event.vcpu as usize], Pending::PageStep { .. })
            && matches!(event.kind, EventType::Singlestep { .. });
        if self.method == Method::PageAccess && page_step && !expected_step {
            self.deferred.push(event);
            return Ok(false);
        }
        match event.kind {
            EventType::Breakpoint { gpa, insn_len: 1 } if gpa == self.target => {
                hits[event.vcpu as usize] += 1;
                match self.method {
                    Method::PageAccess => self.reply(
                        event,
                        EventReplyType::EmulateInsn {
                            data: self.instruction,
                            view: KEEP_MEMORY_VIEW,
                        },
                    )?,
                    Method::MemoryView => {
                        self.drv
                            .toggle_intercept(event.vcpu, InterceptType::Singlestep, true)?;
                        self.pending[event.vcpu as usize] = Pending::ViewStep { write: false };
                        self.reply(
                            event,
                            EventReplyType::Continue {
                                reinject: false,
                                view: DEFAULT_MEMORY_VIEW,
                            },
                        )?;
                    }
                }
                Ok(true)
            }
            EventType::Pagefault { gpa, access, .. } => {
                if gpa / PAGE != self.page / PAGE {
                    return Err(IoError::new(
                        ErrorKind::InvalidData,
                        "page event is outside the target page",
                    )
                    .into());
                }
                if self.method == Method::PageAccess {
                    self.handle_page_access(event, access)?;
                } else {
                    self.handle_view_access(event, access)?;
                }
                Ok(false)
            }
            EventType::Singlestep { .. } => {
                self.handle_step(event)?;
                Ok(false)
            }
            _ => Err(IoError::new(ErrorKind::InvalidData, "unexpected event").into()),
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        if !self.active {
            if self.paused {
                self.drv.resume()?;
                self.paused = false;
            }
            return Ok(());
        }
        if !self.paused {
            self.drv.pause()?;
            self.paused = true;
        }
        if self.pending.iter().any(|pending| {
            matches!(
                pending,
                Pending::PageStep { write: true } | Pending::ViewStep { write: true }
            )
        }) {
            self.drv
                .read_physical(self.page, &mut self.logical, &mut 0)?;
            self.instruction
                .copy_from_slice(&self.logical[self.offset..self.offset + 16]);
        }
        if let Some(view) = self.view {
            self.drv.switch_memory_view(DEFAULT_MEMORY_VIEW)?;
            self.drv.set_page_access(view, self.page, Access::RWX)?;
            self.drv
                .remap_memory_page(view, self.page / PAGE, RESTORE_PAGE)?;
        } else {
            self.drv
                .set_page_access(DEFAULT_MEMORY_VIEW, self.page, Access::RWX)?;
        }
        self.drv.write_physical(self.page, &self.logical)?;
        for vcpu in 0..self.pending.len() {
            self.drv
                .toggle_intercept(vcpu as u16, InterceptType::Singlestep, false)?;
        }
        self.toggle_all(InterceptType::Pagefault, false)?;
        self.toggle_all(InterceptType::Breakpoint, false)?;
        while let Some(event) = self.drv.listen(1)? {
            self.parked.push(event);
        }
        self.parked.append(&mut self.deferred);
        for event in self.parked.drain(..) {
            self.drv.reply_event(
                event,
                EventReplyType::Continue {
                    reinject: false,
                    view: DEFAULT_MEMORY_VIEW,
                },
            )?;
        }
        if let Some(view) = self.view.take() {
            self.drv.destroy_memory_view(view)?;
        }
        if let Some(hidden) = self.hidden.take() {
            self.drv.free_hidden_page(hidden)?;
        }
        if self.method == Method::MemoryView {
            self.drv.enable_memory_views(false)?;
        }
        self.active = false;
        self.drv.resume()?;
        self.paused = false;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("stealth-breakpoint cleanup failed: {}", error);
        }
    }
}

fn run() -> Result<()> {
    let matches = parse_args();
    let method = match matches.value_of("method").unwrap() {
        "page-access" => Method::PageAccess,
        "memory-view" => Method::MemoryView,
        _ => unreachable!(),
    };
    let explicit_gpa = matches.value_of("gpa").map(number).transpose()?;
    let count = number(matches.value_of("count").unwrap())?;

    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))?;

    let drv = microvmi::init(None, Some(DriverInitParams::from_matches(&matches)))?;
    let mut session = Session::new(drv, method, explicit_gpa)?;
    let mut hits = vec![0; session.pending.len()];
    session.start()?;
    let started = Instant::now();
    loop {
        let stop =
            !running.load(Ordering::SeqCst) || (count != 0 && hits.iter().sum::<u64>() >= count);
        if stop
            && session.deferred.is_empty()
            && session
                .pending
                .iter()
                .all(|pending| matches!(pending, Pending::None))
        {
            break;
        }
        if let Some(event) = session.next_event(250)? {
            if session.handle(event, &mut hits)? {
                let total = hits.iter().sum::<u64>();
                println!("Hit {}", total);
            }
        }
    }
    session.cleanup()?;
    let elapsed = started.elapsed().max(Duration::from_nanos(1)).as_secs_f64();
    for (vcpu, hits) in hits.into_iter().enumerate() {
        println!(
            "vCPU {}: {} hits ({:.2}/s)",
            vcpu,
            hits,
            hits as f64 / elapsed
        );
    }
    Ok(())
}

fn main() {
    env_logger::init();
    if let Err(error) = run() {
        eprintln!("stealth-breakpoint: {}", error);
        std::process::exit(1);
    }
}
