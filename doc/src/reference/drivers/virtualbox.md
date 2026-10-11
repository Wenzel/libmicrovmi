# VirtualBox

The VirtualBox driver offers two access methods, selected at init time by `DriverInitParams.virtualbox`. Both name the VM with `common.vm_name`.

| Access method | Cargo feature | Selected by |
|---------------|---------------|-------------|
| MachineDebugger | `virtualbox-api` | `VBoxInitParams::MachineDebugger` |
| FDP (legacy) | `virtualbox` | no `virtualbox` parameter |

## MachineDebugger method

Uses VirtualBox's public `IMachineDebugger` API, loaded at runtime from the VirtualBox installation (no build-time dependency on VirtualBox).

Supported: reading guest physical memory, max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included).
A read that hits an unreadable page (e.g. an MMIO hole) fails, with `bytes_read` set to the bytes read before that page.

Not implemented yet: pause/resume ([#277](https://github.com/Wenzel/libmicrovmi/issues/277)), registers, writes, vCPU count.
Until pause is implemented, tools must not pause the VM (`mem-dump --no-pause`), so they read the memory of a running guest, which may change during the dump.

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
$ ./target/debug/examples/mem-dump --vm_name <vm_name> --vbox_machinedebugger --no-pause
~~~

Python (build the bindings with `--features virtualbox-api`):

~~~
$ python3 python/examples/mem-dump.py -d <vm_name> --vbox-machinedebugger --no-pause
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
