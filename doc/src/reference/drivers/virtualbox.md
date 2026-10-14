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
It is the fast path for VM state access and VMI events. VBoxVMI protocol version 2 is required;
a version 1 client is refused at the handshake.

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
`gpa` points at the next instruction (at RIP); reply with `reinject: false`. Reinjection on a
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

`set_page_access(paddr, access)` restricts the 4K guest RAM page holding `paddr` to the
**allowed** `access`, for every vCPU, as on KVM; `Access::RWX` lifts the restriction.
`get_page_access` returns the allowed access, `RWX` for a page never restricted. Setting is
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
- `EmulateReadData { size, data }` does the same, the instruction's memory reads returning
  `data[..size]` (`size` 1 to 256).
- `EmulateInsn { data }`, at a breakpoint event only, runs the 16 `data` bytes as the instruction
  at RIP instead of guest memory (e.g. the instruction the int3 replaced, without restoring it).

An emulation reply returns once the vCPU ran the instruction. A reply that does not fit the event,
or an emulation VirtualBox cannot perform, fails with the vCPU parked and untouched.

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
