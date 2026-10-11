# Integration tests

Instructions for all tests:
- Update the values in `tests/common/config.rs` according to your environment
- `cargo test -- --nocapture`: displays the `log` output, useful for debugging
- `cargo test --test integration -- <filter>`: runs only the tests whose name contains `<filter>`
- `MICROVMI_TEST_TIMEOUT=<seconds>`: per-test timeout (default: `TIMEOUT` in `tests/common/config.rs`)

## KVM

**Requirements**
- virtual machine already configured to be introspected by KVM-VMI
- VM snapshot with live state
- [libkvmi](https://github.com/bitdefender/libkvmi)
- [`virsh`](https://libvirt.org/manpages/virsh.html) tool: (`libvirt-clients` package)


The VM state between each test is handled by the following commands:
- setup: `virsh snapshot-revert <vm_name> --current --running`
- teardown: `virsh destroy <vm_name>`

**Execution**

~~~
cargo test --feature kvm
~~~

## VirtualBox

The VirtualBox context restores a snapshot, starts the VM headless and creates the driver for one
**access method** (currently the MachineDebugger method, `virtualbox-api` feature). VirtualBox tests
(`virtualbox_*`) check the driver against `VBoxManage` as ground truth, e.g. every RAM page read while
the VM is paused must match a `VBoxManage debugvm dumpvmcore` core file (written to the temp dir,
deleted afterwards).

**Requirements**
- a VirtualBox build with the MachineDebugger patches, its kernel modules loaded
- a VM with an online snapshot (saved state), e.g. a live Alpine ISO guest at its login prompt

**Configuration** (environment variables)
- `VBOX_APP_HOME`: VirtualBox bin directory (`VBoxManage`, `VBoxHeadless`, C API library)
- `VBOX_USER_HOME`: VirtualBox configuration directory holding the VM, if not the default one
- `VBOX_TEST_VM` (default `microvmi-alpine`), `VBOX_TEST_SNAPSHOT` (default `base`)
- `VBOX_TEST_MEMORY_MB`: discard the snapshot's saved state and cold boot the guest with this much RAM
  (e.g. `5120`, to test above 4 GiB). Teardown restores the snapshot, which restores the VM's RAM size.

The VM state between each test is handled by the following commands:
- setup: `controlvm <vm> poweroff` (if running), `snapshot <vm> restore <snapshot>`, `VBoxHeadless --startvm <vm>`
- teardown: `controlvm <vm> poweroff`, `snapshot <vm> restore <snapshot>`

**Execution**

Run the generic tests the method supports, or the VirtualBox ones:

~~~
cargo test --features virtualbox-api --test integration -- virtualbox_
VBOX_TEST_MEMORY_MB=5120 MICROVMI_TEST_TIMEOUT=120 cargo test --features virtualbox-api --test integration -- virtualbox_
~~~

If cargo runs in a container without VirtualBox, build the test binary there and run it on the host:

~~~
cargo test --features virtualbox-api --test integration --no-run   # prints the binary path
VBOX_APP_HOME=/path/to/VirtualBox/bin target/debug/deps/integration-<hash> virtualbox_
~~~

If the VM is shared, serialize runs with `flock -o <lockfile> <command>`: `-o` keeps the lock file
descriptor out of `VBoxHeadless` and `VBoxSVC`, which would otherwise hold the lock after the run.
