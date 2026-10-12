# VirtualBox

The VirtualBox driver offers three access methods, selected at init time by `DriverInitParams.virtualbox`.
MachineDebugger and FDP name the VM with `common.vm_name`; VBoxVMI names it by its socket.

| Access method | Cargo feature | Selected by |
|---------------|---------------|-------------|
| VBoxVMI | `virtualbox-vmi` | `VBoxInitParams::VBoxVMI { socket_path }` |
| MachineDebugger | `virtualbox-api` | `VBoxInitParams::MachineDebugger` |
| FDP (legacy) | `virtualbox` | no `virtualbox` parameter |

## VBoxVMI method

Speaks VBoxVMI, a protocol served from inside the VM process of a patched VirtualBox over a per-VM Unix socket.
It is the fast path, and the way towards VMI events.

Supported: reading guest physical memory, max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included), pause/resume, vCPU count.
Reads are served inside the VM process without waiting for a vCPU, in batches of up to 15 pages per request.
A read that hits an unreadable page (MMIO, including the local APIC page, or no memory) fails, with `bytes_read` set to the bytes read before that page; devices are never accessed.
Pause is VirtualBox's own pause (the VM shows as Paused in the GUI and `VBoxManage`). It is synchronous, all vCPUs are stopped when it returns, and idempotent: pausing a paused VM succeeds, and one resume resumes it.
Pauses are not counted, also across clients: resume resumes the VM even if the GUI, `VBoxManage` or another tool paused it too.
Pause and resume fail while the VM is neither running nor paused (powering on or off, saving or restoring its state, live snapshot, teleport), and resume fails while the host's power management has the VM paused.
A VM paused through VBoxVMI stays paused if the tool exits or disconnects without resuming it ([#287](https://github.com/Wenzel/libmicrovmi/issues/287)).

Not implemented yet: registers, writes ([#284](https://github.com/Wenzel/libmicrovmi/issues/284)–[#285](https://github.com/Wenzel/libmicrovmi/issues/285)): they return a "not implemented yet" error.

### Requirements

- Platform: Linux
- A VirtualBox built with VBoxVMI (`VBOX_WITH_VMI`, branch `vmi/vboxvmi` of [Wenzel/virtualbox](https://github.com/Wenzel/virtualbox); protocol and design in its `doc/technical/VBoxVMI-*.md`).
- VBoxVMI enabled on the VM before it starts: `VBoxManage setextradata <vm> VBoxInternal/VMI/Enabled 1`.
  The socket is `<VirtualBox user home>/vmi/<VM UUID>.sock` (e.g. `~/.config/VirtualBox/vmi/`), or the path set in `VBoxInternal/VMI/SocketPath`.
- libmicrovmi runs as the user running the VM (the socket is mode 0600 and the server checks the peer's uid; root is refused too).
- A custom `SocketPath` must be absolute, in a directory owned by that user and not writable by group or others (unless sticky); otherwise the VM refuses to start.
- One client per VM at a time: a second client gets a "another VBoxVMI client is already connected" error.

### Initialization parameters

- `vm_name`: ignored (the socket belongs to one VM)
- `virtualbox`: `VBoxInitParams::VBoxVMI { socket_path }`
  - Python: `init_params.virtualbox = VBoxInitParamsPy.VBoxVMI(socket_path="...")`
  - C: `.virtualbox = { .access_method = VBOX_ACCESS_METHOD_VBOXVMI, .socket_path = "..." }` (see `c_examples/vbox-read-page.c`)

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
- A VirtualBox whose `IMachineDebugger::readPhysicalMemory` is implemented. Stock VirtualBox releases return "not implemented"; it needs a VirtualBox built with the IMachineDebugger memory methods patch.
- The VM is running, and owned by the user running libmicrovmi (the API attaches a shared session through that user's `VBoxSVC`).
- The VirtualBox C API library `VBoxXPCOMC.so` is found in `VBOX_APP_HOME`, else in `/opt/VirtualBox` or `/usr/lib/virtualbox`. Set `VBOX_APP_HOME` to the directory of a VirtualBox that is not installed (e.g. a build output `out/linux.amd64/release/bin`).
- The driver must be used on the thread that initialized it.

### Initialization parameters

- `vm_name`: required
- `virtualbox`: `VBoxInitParams::MachineDebugger`
  - Python: `init_params.virtualbox = VBoxInitParamsPy.MachineDebugger()`
  - C: `.virtualbox = { .access_method = VBOX_ACCESS_METHOD_MACHINE_DEBUGGER }` (`VBOX_ACCESS_METHOD_NONE`, the zero value, means no `VBoxInitParams`; see `c_examples/vbox-read-page.c`)

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

## FDP method (legacy)

### Requirements

- virtualbox modified via [icebox](https://github.com/thalium/icebox) project
- Platform: Windows/Linux

To compile `libFDP`
~~~
$ git clone --depth 1 https://github.com/thalium/icebox
$ cd icebox/src/FDP
$ g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so
$ sudo mv include/* /usr/local/include/
$ sudo mv libFDP.so /usr/local/lib/
~~~

### Initialization parameters

- `vm_name`: required
