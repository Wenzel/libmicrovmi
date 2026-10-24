# VirtualBox

The VirtualBox driver offers two access methods, selected at init time by
`DriverInitParams.virtualbox`. MachineDebugger names the VM with `common.vm_name`; VBoxVMI names it
by its socket, or by `common.vm_name` for the VM's default socket. The `virtualbox` Cargo feature
enables both methods on Unix; the method-specific features remain available separately.

VirtualBox has one EFER register: `X86Registers::efer` and `msr_efer` both read it, and a register
write with MachineDebugger or VBoxVMI fails (before writing anything) unless they are equal.

| Access method | Cargo feature | Selected by |
|---------------|---------------|-------------|
| VBoxVMI | `virtualbox-vmi` or `virtualbox` | `VBoxInitParams::VBoxVMI { socket_path }` |
| MachineDebugger | `virtualbox-api` or `virtualbox` | `VBoxInitParams::MachineDebugger` |

## Default access method

Without `DriverInitParams.virtualbox`, the driver uses the VBoxVMI method at the VM's default socket
(`<VirtualBox user home>/vmi/<VM UUID>.sock`, or the VM's `VBoxInternal/VMI/SocketPath`, both
resolved by `common.vm_name` through the VirtualBox API), and falls back to the MachineDebugger
method only if that socket file does not exist, i.e. VBoxVMI is not enabled on the VM. Any other
VBoxVMI error (another client connected, permission refused, protocol mismatch) is returned. An
explicitly selected method never falls back. The choice, and the reason for a fallback, are logged at
`info` level (`RUST_LOG=microvmi=info`). See [ADR-0004](https://github.com/Wenzel/libmicrovmi/blob/dev/docs/adr/0004-default-vbox-access-method.md).

With one method compiled in: `virtualbox-api` only uses MachineDebugger by default;
`virtualbox-vmi` only needs an explicit `socket_path`.

## VBoxVMI method

Speaks VBoxVMI, a protocol served from inside the VM process of a patched VirtualBox over a per-VM Unix socket.
It is the fast path for VM state access and VMI events. The client and patched server must agree
on the VBoxVMI protocol version (9 in the measured builds); a mismatch is refused at the handshake.

Supported: reading and writing guest physical memory, reading and writing the complete x86 register set,
max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included),
pause/resume, vCPU count, page access restrictions, and breakpoint (int3), single-step, CR, MSR
and page-access intercepts per vCPU.
Physical reads and writes are served inside the VM process without waiting for a vCPU, batching pages into requests of at most 64 KiB.
A read that hits an unreadable page (MMIO, including the local APIC page, or no memory) fails, with `bytes_read` set to the bytes read before that page; devices are never accessed.
A physical write to a non-RAM page fails; other pages in the same batch may already have been written, so writes are not atomic. MMIO devices are never accessed.
Pause is VirtualBox's own pause (the VM shows as Paused in the GUI and `VBoxManage`). It is synchronous, all vCPUs are stopped when it returns, and idempotent: pausing a paused VM succeeds, and one resume resumes it.
Pauses are not counted, also across clients: resume resumes the VM even if the GUI, `VBoxManage` or another tool paused it too.
Pause and resume fail while VirtualBox controls the VM's run state: powering on or off, saving or restoring its state, live snapshot, teleport, or a configuration change (medium or network attachment). Resume also fails while the host's power management has the VM paused, and after a failed teleport of a paused VM until it is resumed through VirtualBox.
A client disconnect leaves no session state behind: the server clears its intercepts, releases
parked vCPUs without reinjection and synchronously resumes a pause the client created. A pause
that already existed, or an external pause after the client's pause was externally resumed, remains paused.
A register read or write is one request, served by the vCPU itself between two of its instructions: a read is a consistent snapshot of that vCPU and a write changes all its registers at once, also while the VM runs (pause first to read or write several vCPUs at the same point).
The MSRs of `X86Registers` (EFER, STAR, LSTAR, SYSENTER_CS/ESP/EIP, APIC base) are included; an MSR the guest CPU doesn't have reads as 0.
A write is all or nothing: a value the CPU itself would not load (RFLAGS with bit 1 clear or a reserved bit set, reserved bits in CR0, CR3, CR4 or EFER, an inconsistent long mode state, an MSR value `WRMSR` refuses) fails the whole write, and nothing is written. Registers keeping their value are not touched, so writing back registers read earlier always succeeds and has no side effects.
Segment selectors, bases and limits are written as raw state: no descriptor table load, the segment attributes are kept, and keeping them consistent is up to the caller. Writes fail while the VM is neither running nor paused (powering on or off, saving or restoring its state, live snapshot, teleport).

### VMI events

`toggle_intercept(vcpu, intercept, enabled)` sets or clears an intercept, also while paused or
while handling an event on that vCPU: `InterceptType::Breakpoint`, `InterceptType::Singlestep`,
`InterceptType::Cr(CrType::Cr0 | Cr3 | Cr4)` (writes to that control register),
`InterceptType::Msr(index)` (writes to that MSR) and `InterceptType::Pagefault` (page-access
events). `listen(timeout_ms)` returns the oldest pending
event across vCPUs or `None` after the timeout. Each reported event parks only its vCPU; other
vCPUs continue. The client can read and write the parked vCPU's registers and guest memory, and
pause/resume keeps it parked.

At a breakpoint event `gpa` is the physical address of the int3, `insn_len` is its length and RIP
points at the int3. Restore the original byte before
`reply_event(event, EventReplyType::Continue { reinject: false })` to execute the original
instruction, or choose `reinject: true` to deliver the guest's own #BP. A single-step event's
`gpa` points at the next instruction (at RIP); reply with `reinject: false`. As on Xen, a stepping
vCPU gets no external interrupt until stepping stops, so a step never lands in an interrupt
handler. Reinjection on a
non-breakpoint event fails without releasing the vCPU. A reply for an unparked vCPU or a second
reply fails. Reply while the VM is paused succeeds; execution resumes when the VM does. A reply
that fails leaves the vCPU parked and untouched, so the client can reply again.

CR and MSR write intercepts need VT-x; setting one on AMD-V or NEM fails with "not supported".
A vCPU holds at most 64 MSR intercepts; setting more fails. Only the intercepted vCPU and
register exit: the other vCPUs, MSR reads and other MSRs run at full speed. Writes are reported
before they take effect: every `MOV CR3` and `WRMSR` (also unchanged values and MSRs VirtualBox
does not implement), and every `MOV CR0`/`MOV CR4` that changes the register, including bits
VirtualBox normally leaves to the guest (VT-x does not exit on an unchanged CR0/CR4 write;
`CLTS` and `LMSW` are not reported). RIP points at the `MOV CRx` or `WRMSR`, the register still
holds `old`, and after `reply_event(event, EventReplyType::Continue { reinject: false })` the
instruction executes unchanged (change the source register first to write another value). A CR
event's `new` is the value the register will hold (CR3: bit 63, the PCID no-flush bit,
cleared); an MSR event's `value` is EDX:EAX and `old` is the MSR's current value, 0 for an MSR
VirtualBox does not implement (the guest then gets its #GP). With single step also set, the CR
or MSR event comes first, then the single-step event after the instruction.

VBoxVMI intercepts and the VirtualBox debugger exclude each other: setting an intercept while
the debugger is attached fails; debugger attachment while any intercept is set fails. Clear the
intercepts or disconnect before attaching the debugger. See [ADR-0005](https://github.com/Wenzel/libmicrovmi/blob/dev/docs/adr/0005-vboxvmi-events-reinjection-and-debugger-exclusion.md).

#### Page access

`set_page_access(view, paddr, access)` restricts the 4K guest RAM page holding `paddr` to the
**allowed** `access` in Memory view `view`, for every vCPU whose Active view is `view`, as on KVM;
`Access::RWX` lifts the restriction. `get_page_access(view, paddr)` returns the allowed access in
that view, `RWX` for a page never restricted there. Setting is
idempotent and works while paused and while handling an event, before the reply. It fails for
write without read (`W`, `WX`), execute-only (`X`) on a CPU whose EPT cannot enforce it, MMIO, ROM
and unbacked addresses, and while the VirtualBox debugger is attached; while any restriction is
set the debugger cannot attach (as for intercepts). Page access and the page-access intercept need
VT-x ("not supported" on AMD-V and NEM). Only vCPU accesses are restricted (also VirtualBox's
emulation of vCPU instructions); device, DMA and the client's own physical reads and writes are
not. Disconnect removes every restriction.

Only vCPUs with `InterceptType::Pagefault` report a denied access; the others perform it
unreported. An `EventType::Pagefault` event is reported before the access, RIP at the faulting
instruction: `gpa` is the full guest-physical address, `gva` the linear address (0 when the CPU
reported none), `access` the single access attempted (`R`, `W` or `X`). It comes before the
breakpoint and single-step events of the same instruction; an instruction touching several
restricted pages raises one event per retry. Replies (ADR-0007):

- `Continue { reinject: false }` retries the access: it faults again unless the restriction was
  lifted (or the vCPU steps). `reinject: true` fails.
- `Emulate` runs the instruction once with every restriction ignored, keeping them set.
- `EmulateNoWrite` runs the instruction once but discards its memory writes, keeping
  restrictions set.
- `EmulateReadData { size, data }` does the same, the instruction's memory reads returning
  `data[..size]` (`size` 1 to 256).
- `EmulateInsn { data }`, at a breakpoint event only, runs the 16 `data` bytes as the instruction
  at RIP instead of guest memory (e.g. the instruction the int3 replaced, without restoring it).

An emulation reply returns once the vCPU ran the instruction. A reply that does not fit the event,
or an emulation VirtualBox cannot perform, fails with the vCPU parked and untouched.

#### Memory views

`enable_memory_views(true)` turns on Memory views for the VM (fails while the debugger is
attached, like an Intercept); every vCPU starts in the Default view (`DEFAULT_MEMORY_VIEW`, 0).
`create_memory_view()` allocates a view and returns its number; `destroy_memory_view(view)` frees
one that is no vCPU's Active view. `switch_memory_view(view)` moves every vCPU to `view`.
`enable_memory_views(false)` switches every vCPU back to Default, destroys every other view and
lifts every restriction.

Each view has its own page-access restrictions and Remaps: `remap_memory_page(view, page, target)`
makes `page` (a page number, not a byte address) show the contents of `target`'s page in `view`
only; `target: RESTORE_PAGE`-equivalent (pass `page` itself) undoes it. A Remap is not available
in the Default view. Every `Event` carries the `view` its vCPU was running in; an event reply's
`view` field switches that vCPU to another view (`KEEP_MEMORY_VIEW` keeps its Active view, the
usual choice). A view that is some vCPU's Active view cannot be destroyed.

`allocate_hidden_page()`/`free_hidden_page(page)` hand out pages from a pool reserved when the VM
starts (`VBoxInternal/VMI/HiddenPages`, a page count, default 0), for Remap targets: content only
a Remap can expose, invisible in the Default view and absent from the RAM ranges VBoxVMI reports.
A common pattern (a stealth breakpoint) remaps the int3's page to a Hidden page holding the
patched byte, so the Default view (and the client's own physical reads) keep seeing the original
byte.

The client's own physical reads and writes always use the Default view, regardless of any vCPU's
Active view.

#### Hidden page pool sizing

The Hidden page pool size is fixed at cold boot via `VBoxInternal/VMI/HiddenPages`. Changing it
invalidates existing saved states and snapshots. DRAKVUF needs 1 sink page plus 1 page per hooked
page. Measured counts (Linux 6.8 guest, Intel VT-x):

- syscalls plugin: 169 pages (1 sink + 168 shadows)
- ptracemon plugin: 2 pages (1 sink + 1 shadow)
- procmon plugin: 5 pages (1 sink + 4 shadows)

DRAKVUF reports the pool size and usage at startup ("X/Y used"). On pool exhaustion,
`allocate_hidden_page()` fails and DRAKVUF aborts initialization with a sizing hint.

**Register round-trip elimination (DRAKVUF #12, 2026-10-24).** VBoxVMI protocol v10 events
(VirtualBox `99b30a9aa0d`, rs-vboxvmi `1845439`, libmicrovmi `3fe13ce`, LibVMI `d1a240a`) carry
the vCPU's full register snapshot (VMIX86REGS, 416 bytes) in the event message itself,
eliminating the separate `microvmi_read_registers` socket round trip LibVMI used to perform
per event. Baseline (protocol v9, eBPF uprobes): 2.0 `microvmi_read_registers` calls per INT3
event on the ptracemon path (both from `microvmi_process_event` filling `event->x86_regs` and
the per-vCPU register cache). Protocol v10 (same probe): 5 calls across 7100 INT3 events
(0.00 per event); LibVMI seeds the cache directly from the event via `microvmi_event_get_registers()`.
Four host-bracketed 25-command runs (same two-vCPU VM, strace workload as #11) initially measured
36.39–36.86 s (median 36.495 s), 95.6–97.5 INT3/s, delivering 3525–3550 ptracemon events (median
3537.5). Investigation revealed ~26 `microvmi_listen` calls per run timed out after 500 ms
without receiving an event, totaling ~20 s of the measured time. Root cause: when multiple events
arrived in quick succession, rs-vboxvmi's `wait_event` returned the first but left others on the
socket; the next `wait_event` call would poll with a full timeout even though events were
immediately available. Fixed by draining all immediately-available events into the internal queue
before returning (rs-vboxvmi `17b4d02`). Post-fix measurements on the same build and host:
6.8–17.5 s (median 10.3 s), ~340 INT3/s, delivering 3526–3550 ptracemon events. This is faster
than the protocol v9 baseline (5.119–17.993 s, median 11.018 s, 197–693 INT3/s) measured on the
same host. The register round-trip elimination is confirmed (2.0 → 0.00 `microvmi_read_registers`
calls per event). The register cache invalidation paths (set_vcpureg(s), SET_REGISTERS reply,
unpark, resume) remain unchanged. PIDs in ptracemon output were correct (seeded registers drive
process lookup via shadow_gs/CR3, not affected by the optimization).

#### Measured event latency (DRAKVUF #11, 2026-10-24)

**Reproduction.** Host: Linux 6.8.0-146-generic, Intel Core i9-14900K (24 cores,
32 hardware threads), VT-x/EPT; VirtualBox `2fc9912442b`, rs-vboxvmi `c0529e3`,
libmicrovmi `047086a`, LibVMI `9d7355a`, DRAKVUF `95b514f`, VBoxVMI protocol 9.
Guest: Ubuntu 24.04, Linux 6.8.0-146-generic, 2 GiB, NAT/SSH, base snapshot,
one or two vCPUs, VMI enabled at cold boot. Xen was unavailable on this host.
For each session, run
`drakvuf -r ubuntu/linux-6.8.0-146-generic.json -d vbox:microvmi-ubuntu -a ptracemon [-F] -t 35`,
wait 4 s for setup, and execute over SSH
`for i in $(seq 25); do strace -f -qq /bin/true >/dev/null 2>&1 || exit 13; done`.
Bracket the SSH command with host timestamps; stop DRAKVUF immediately after completion,
then check SSH, guest `dmesg --level=err,warn`, and VBox.log. On two vCPUs,
20 traced sessions ran in five `normal, -F, -F, normal` blocks across two fresh
boots; on one vCPU, four traced sessions ran in one such block on a fresh boot.
Every run completed 25 commands and delivered 3550 ptracemon INT3 callbacks:
normal had another 3550 public single-step events, whereas `-F` had none.
An unmonitored 25-command run took 0.061 s guest wall time on two vCPUs and
0.035 s on one vCPU (shell `time`). The four-session, two-vCPU no-eBPF
calibration also spanned 5.58–15.68 s under DRAKVUF: tracing alone does not
explain the large run-to-run variation.

**Probe boundaries.** Throwaway bpftrace uprobes on the deployed binaries
timestamped `VMIR3DebugEvent` entry/return (park, keyed by EMT), entry to
`vmiR3Send` (event dispatch/queue, matched by the event's vCPU), send return to
`microvmi_listen` return (socket reception and libmicrovmi event translation),
and `microvmi_reply_event` entry/return (reply plus server acknowledgment).
`microvmi_listen` return to `microvmi_process_event` entry measures LibVMI's
cache flush and routing; `microvmi_process_event` entry/return includes the
DRAKVUF callback and reply. `_int3_cb` entry/return isolates the DRAKVUF
INT3 callback, and `pgmVmiSwitchEptView` entry/return measures one view switch.
The send syscall itself was measured by the server's `vmiR3Send` entry/return.
Match events by vCPU, not alternating samples: normal's INT3/step sequence
otherwise biases percentiles. The park span includes all downstream stages;
LibVMI and callback spans overlap and **must not be added**. No guest
instruction timing or VM-exit-to-`VMIR3DebugEvent` time is implied.
The values below are approximate p50/p95/p99 **microseconds**, rounded to
the containing eight-buckets-per-octave histogram bin; all spans include
eBPF probe cost. Two-vCPU spans aggregate the last 12 sessions (six per mode,
with all socket probes validated against event counts); one-vCPU spans
aggregate all four sessions (two per mode).

| Stage, µs p50 / p95 / p99 | 1 vCPU normal | 1 vCPU `-F` | 2 vCPU normal | 2 vCPU `-F` |
| --- | ---: | ---: | ---: | ---: |
| VBox EMT park to resume, INT3 | 736 / 3968 / 4352 | 672 / 736 / 864 | 928 / 4864 / 6400 | 864 / 4352 / 5376 |
| VBox dispatch to event send, INT3 | 3 / 19 / 29 | 3 / 4 / 7 | 4 / 23 / 34 | 3 / 23 / 31 |
| Socket event send to libmicrovmi receive/translate, INT3 | 1 / 10 / 19 | 1 / 2 / 4 | 2 / 13 / 31 | 1 / 11 / 21 |
| Server event send syscall | 1 / 6 / 9 | 1 / 1 / 2 | 1 / 7 / 13 | 1 / 7 / 10 |
| LibVMI pre-callback cache flush/routing | 5 / 27 / 38 | 5 / 6 / 9 | 5 / 38 / 62 | 6 / 38 / 58 |
| LibVMI dispatch **including** callback/reply | 672 / 3456 / 3968 | 672 / 736 / 864 | 800 / 4352 / 5376 | 800 / 4352 / 5376 |
| DRAKVUF INT3 callback | 672 / 3712 / 4352 | 672 / 736 / 864 | 928 / 4352 / 5888 | 800 / 4352 / 4864 |
| Reply socket round trip, including acknowledgment | 10 / 58 / 84 | 11 / 12 / 17 | 12 / 68 / 108 | 11 / 68 / 152 |
| One EPT view switch | 0 / 5 / 7 | 1 / 1 / 1 | 1 / 6 / 12 | 1 / 5 / 11 |

**Throughput and guest slowdown.** On two vCPUs, normal host-bracketed
25-command times were 5.119–17.993 s (median 11.018 s), or 197–693
INT3/s; `-F` was 5.200–10.130 s (median 7.898 s), or 350–683 INT3/s.
The ten ordered normal-minus-fast pairs ranged from **-0.799 to +8.915 s**
(median +2.385 s; eight positive, two negative), so this does **not**
demonstrate a repeatable guest speedup. Guest `CLOCK_MONOTONIC` measurements
on the last ABBA block were normal 10.672 and 17.703 s, `-F` 7.285 and
8.802 s, versus 0.071 s unmonitored with the same monotonic-clock wrapper;
host-bracketed times for these four were 11.180, 17.993, 7.594 and 9.079 s
respectively. Treat the 0.28–0.51 s host/guest clock discrepancy as a
measurement limit rather than a claimed speedup. On one vCPU, host-bracketed
normal times were 8.199 and 6.462 s (433–549 INT3/s), versus `-F` 4.370
and 4.317 s (812–822 INT3/s); two runs are insufficient to extrapolate.
The extra normal single-step events double its total event/reply rate, not
its count of ptracemon hits.

The INT3 callback/LibVMI dispatch dominates the parked interval (roughly
0.7–1 ms median and 4–6 ms at p99 on two vCPUs); queueing, socket delivery,
reply acknowledgment and individual view switches are one to two orders of
magnitude smaller. `-F` removes the public step/reply but does not remove
the measured multi-millisecond callback tail on two vCPUs. The conflicting
no-probe controls and wide paired spread do not isolate a deterministic
performance defect in `-F`; this benchmark provides no basis for changing
the unrelated remap/page-access paths. SSH stayed responsive after every
session, and dmesg/VBox.log showed no run-time soft lockups, RCU stalls, I/O
errors, assertions or VMI failures. Boot-time APIC calibration and snapshot
journal-recovery warnings were present before tracing; clocksource warning
tracking remains in DRAKVUF #28. No raw trace files were committed.


### Requirements

- Platform: Linux
- A VirtualBox built with VBoxVMI (`VBOX_WITH_VMI`, branch `vmi/vboxvmi` of [Wenzel/virtualbox](https://github.com/Wenzel/virtualbox); protocol and design in its `doc/technical/VBoxVMI-*.md`).
- VBoxVMI enabled on the VM before it starts: `VBoxManage setextradata <vm> VBoxInternal/VMI/Enabled 1`.
  The socket is `<VirtualBox user home>/vmi/<VM UUID>.sock` (e.g. `~/.config/VirtualBox/vmi/`), or the path set in `VBoxInternal/VMI/SocketPath`.
- libmicrovmi runs as the user running the VM (the socket is mode 0600 and the server checks the peer's uid; root is refused too).
- A custom `SocketPath` must be absolute, in a directory owned by that user and not writable by group or others (unless sticky); otherwise the VM refuses to start.
- One client per VM at a time: a second client immediately gets the error "another VBoxVMI client is already connected".

### Initialization parameters

- `vm_name`: required when `socket_path` is omitted, ignored otherwise (the socket belongs to one VM)
- `virtualbox`: `VBoxInitParams::VBoxVMI { socket_path: Some(path) }`, or `socket_path: None` for the
  VM's default socket (needs the `virtualbox` feature: the socket is resolved through the VirtualBox API)
  - Python: `init_params.virtualbox = VBoxInitParamsPy.VBoxVMI(socket_path="...")`, or `VBoxVMI()`
  - C: `.virtualbox = { .access_method = VBOX_ACCESS_METHOD_VBOXVMI, .socket_path = "..." }` (see `c_examples/vbox-read-page.c`), or a null `socket_path`

The examples take `--vbox_vmi <socket>`:
~~~
$ cargo build --features virtualbox-vmi --examples
$ ./target/debug/examples/mem-dump --vbox_vmi ~/.config/VirtualBox/vmi/<vm uuid>.sock
~~~

Python (bindings built with the `virtualbox-vmi` feature):
~~~
$ python3 python/examples/mem-dump.py --vbox-vmi ~/.config/VirtualBox/vmi/<vm uuid>.sock
~~~

The client comes from the [vboxvmi](https://github.com/Wenzel/rs-vboxvmi) crate. To build against a local checkout of it:
`cargo build --features virtualbox-vmi --config 'patch."https://github.com/Wenzel/rs-vboxvmi".vboxvmi.path="../rs-vboxvmi"'`

### DRAKVUF proof of concept

DRAKVUF runs on a Linux guest through LibVMI's libmicrovmi driver and VBoxVMI, without Xen
([#361](https://github.com/Wenzel/libmicrovmi/issues/361)). Verified with these revisions:

| Component | Repository, branch | Revision |
|-----------|--------------------|----------|
| VirtualBox | [Wenzel/virtualbox](https://github.com/Wenzel/virtualbox) `vmi/vboxvmi` | `5656fb6754fe8d9062c8121cbcd6bd233cabda89` |
| rs-vboxvmi | [Wenzel/rs-vboxvmi](https://github.com/Wenzel/rs-vboxvmi) `memory-views` | `6a076b8fc1c44464a0cdd23ba1baf8363f71becc` |
| libmicrovmi | Wenzel/libmicrovmi `dev` | `e50934fd2c861dd7bc3aeb5b4f550b3147abbfa9` |
| LibVMI | [Wenzel/libvmi](https://github.com/Wenzel/libvmi) `libmicrovmi` | `8709db25c93af6528c360b57ddbfa888031e4896` |
| DRAKVUF | [tklengyel/drakvuf](https://github.com/tklengyel/drakvuf) `main` + commit | `b0aee4b7e6789f03ea8873f7b0ffd5e169bb446f` |

Guest: Ubuntu 24.04, kernel 6.8.0-146-generic, 2 vCPUs, 2 GiB, VT-x with nested paging, and a
Volatility 3 ISF for its kernel (`linux-6.8.0-146-generic.json`).

1. Build and install libmicrovmi with `--features virtualbox`, then LibVMI against it, then
   DRAKVUF against that LibVMI (`meson setup build && ninja -C build`; DRAKVUF still links its Xen
   libraries, VirtualBox mode never calls them).
2. Before a cold start of the VM:
   ~~~
   $ VBoxManage setextradata <vm> VBoxInternal/VMI/Enabled 1
   $ VBoxManage setextradata <vm> VBoxInternal/VMI/HiddenPages 64
   ~~~

3. Run as the VM's user:
   ~~~
   $ drakvuf -r linux-6.8.0-146-generic.json -d vbox:<vm> -a ptracemon -o json -t 60 -v
   ~~~
4. Once DRAKVUF prints "Started DRAKVUF polling loop" (`-v`), trigger ptrace in the guest, e.g. a
   Python `ctypes` script forking a child, then `ptrace(PTRACE_ATTACH, child)`, `waitpid`,
   `ptrace(PTRACE_DETACH, child)`.
   Expected: one record per call, e.g.
   `{"Plugin":"ptracemon",...,"Method":"ptrace",...,"Type":"PTRACE_ATTACH","TargetPID":1512,...}`
   then `"PTRACE_DETACH"`; the guest script finishes while DRAKVUF runs, DRAKVUF exits 0 on
   SIGINT or timeout, and the VM keeps running and answering afterwards.

With too few Hidden pages DRAKVUF prints "Failed to allocate a Hidden page", ptracemon fails to
start and DRAKVUF exits non-zero after removing its views. `-F` and `-p` are Xen-only.

## MachineDebugger method

Uses VirtualBox's public `IMachineDebugger` API, loaded at runtime from the VirtualBox installation (no build-time dependency on VirtualBox).

Supported: reading guest physical memory, reading and writing the complete x86 register set,
max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included),
pause/resume, and vCPU count. A memory read that hits an unreadable page (e.g. an MMIO hole)
fails, with `bytes_read` set to the bytes read before that page.
Pause is VirtualBox's own pause (the VM shows as Paused in the GUI and `VBoxManage`). It is synchronous, all vCPUs are stopped when it returns, and idempotent: pausing a paused VM succeeds, and one resume resumes it.
Pauses are not counted, also across clients: resume resumes the VM even if the GUI, `VBoxManage` or another tool paused it too.
Pause and resume fail while a live snapshot or teleport is in progress, during which VirtualBox controls the VM's run state.
With CPU hot-plug enabled, the vCPU count is the VM's maximum vCPU count, not the vCPUs currently plugged in.

Not implemented yet: writing guest physical memory. VMI events are not supported by this access
method; selecting MachineDebugger rather than VBoxVMI for an event request returns an error.

### Requirements

- Platform: Linux
- A VirtualBox whose `IMachineDebugger::readPhysicalMemory` is implemented. Stock VirtualBox
  releases return "not implemented"; it needs a VirtualBox built with the IMachineDebugger memory
  methods patch.
- The VM is running and owned by the user running libmicrovmi (the API attaches a shared session
  through that user's VirtualBox service).
- The VirtualBox C API runtime must be installed. On Linux, `VBoxXPCOMC.so` is found in
  `VBOX_APP_HOME`, `/opt/VirtualBox` or `/usr/lib/virtualbox`; set `VBOX_APP_HOME` to a build output
  such as `out/linux.amd64/release/bin`.
- The driver must be used on the thread that initialized it.

### Initialization parameters

- `vm_name`: required
- `virtualbox`: `VBoxInitParams::MachineDebugger`
  - Python: `init_params.virtualbox = VBoxInitParamsPy.MachineDebugger()`
  - C: `.virtualbox = { .access_method = VBOX_ACCESS_METHOD_MACHINE_DEBUGGER }` (see `c_examples/vbox-read-page.c`)

~~~
$ cargo build --features virtualbox-api --examples
$ ./target/debug/examples/mem-dump --vm_name <vm_name> --vbox_machinedebugger
~~~

Python (build the bindings with `--features virtualbox-api`):

~~~
$ python3 python/examples/mem-dump.py -d <vm_name> --vbox-machinedebugger
~~~

The binding comes from the [vboxapi](https://github.com/Wenzel/rs-vboxapi) crate. To build against a local checkout of it, override the git dependency:
`cargo build --features virtualbox-api --config 'patch."https://github.com/Wenzel/rs-vboxapi".vboxapi.path="../rs-vboxapi"'`
