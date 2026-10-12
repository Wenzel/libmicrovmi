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
It is the fast path, and the way towards VMI events.

Supported: reading and writing guest physical memory, reading and writing the complete x86 register set, max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included), pause/resume, vCPU count.
Physical reads and writes are served inside the VM process without waiting for a vCPU, batching pages into requests of at most 64 KiB.
A read that hits an unreadable page (MMIO, including the local APIC page, or no memory) fails, with `bytes_read` set to the bytes read before that page; devices are never accessed.
A physical write to a non-RAM page fails; other pages in the same batch may already have been written, so writes are not atomic. MMIO devices are never accessed.
Pause is VirtualBox's own pause (the VM shows as Paused in the GUI and `VBoxManage`). It is synchronous, all vCPUs are stopped when it returns, and idempotent: pausing a paused VM succeeds, and one resume resumes it.
Pauses are not counted, also across clients: resume resumes the VM even if the GUI, `VBoxManage` or another tool paused it too.
Pause and resume fail while VirtualBox controls the VM's run state: powering on or off, saving or restoring its state, live snapshot, teleport, or a configuration change (medium or network attachment). Resume also fails while the host's power management has the VM paused, and after a failed teleport of a paused VM until it is resumed through VirtualBox.
A client disconnect leaves no session state behind: the server removes its intercepts and synchronously resumes a pause the client created. A pause that already existed, or an external pause after the client's pause was externally resumed, remains paused.
A register read or write is one request, served by the vCPU itself between two of its instructions: a read is a consistent snapshot of that vCPU and a write changes all its registers at once, also while the VM runs (pause first to read or write several vCPUs at the same point).
The MSRs of `X86Registers` (EFER, STAR, LSTAR, SYSENTER_CS/ESP/EIP, APIC base) are included; an MSR the guest CPU doesn't have reads as 0.
A write is all or nothing: a value the CPU itself would not load (RFLAGS with bit 1 clear or a reserved bit set, reserved bits in CR0, CR3, CR4 or EFER, an inconsistent long mode state, an MSR value `WRMSR` refuses) fails the whole write, and nothing is written. Registers keeping their value are not touched, so writing back registers read earlier always succeeds and has no side effects.
Segment selectors, bases and limits are written as raw state: no descriptor table load, the segment attributes are kept, and keeping them consistent is up to the caller. Writes fail while the VM is neither running nor paused (powering on or off, saving or restoring its state, live snapshot, teleport).

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

Not implemented yet: writing guest physical memory.

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
