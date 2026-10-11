# VirtualBox

The VirtualBox driver offers two access methods, selected at init time by `DriverInitParams.virtualbox`:

| Access method | Cargo feature | Selected by |
|---------------|---------------|-------------|
| MachineDebugger | `virtualbox-api` | `VBoxInitParams::MachineDebugger { vm_name }` |
| FDP (legacy) | `virtualbox` | no `virtualbox` parameter, `common.vm_name` |

## MachineDebugger method

Uses VirtualBox's public `IMachineDebugger` API, loaded at runtime from the VirtualBox installation (no build-time dependency on VirtualBox).

Supported: reading guest physical memory, max physical address (end of the highest guest RAM range, so RAM above 4 GiB is included).

### Requirements

- Platform: Linux
- A VirtualBox whose `IMachineDebugger::readPhysicalMemory` is implemented. Stock VirtualBox releases return "not implemented"; it needs a VirtualBox built with the IMachineDebugger memory methods patch.
- The VM is running, and owned by the user running libmicrovmi (the API attaches a shared session through that user's `VBoxSVC`).
- The VirtualBox C API library `VBoxXPCOMC.so` is found in `VBOX_APP_HOME`, else in `/opt/VirtualBox` or `/usr/lib/virtualbox`. Set `VBOX_APP_HOME` to the directory of a VirtualBox that is not installed (e.g. a build output `out/linux.amd64/release/bin`).
- The driver must be used on the thread that initialized it.

### Initialization parameters

- `virtualbox`: `VBoxInitParams::MachineDebugger { vm_name }`, the name of the VM

~~~
$ cargo build --features virtualbox-api --examples
$ ./target/debug/examples/mem-dump --vbox_machinedebugger <vm_name> --no-pause
~~~

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
