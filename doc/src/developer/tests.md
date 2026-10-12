# Integration tests

Instructions for all tests:
- Update the values in `tests/common/config.rs` according to your environment
- `cargo test -- --nocapture`: displays the `log` output, useful for debugging
- `MICROVMI_TEST_FILTER=<filter>`: runs only the tests whose name contains `<filter>` (none: the run fails)
- `MICROVMI_TEST_TIMEOUT=<seconds>`: per-test timeout (default: `TIMEOUT` in `tests/common/config.rs`,
  plus 10s per GiB of guest RAM for VirtualBox). A test that times out can't be stopped, so the run
  aborts after its teardown and the remaining tests don't run.
- the run exits non-zero if a test or its setup fails

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
**access method**, chosen by `VBOX_TEST_METHOD`: the MachineDebugger method (`virtualbox-api` feature)
or the VBoxVMI method (`virtualbox-vmi` feature). VirtualBox tests
(`virtualbox_*`) check the driver against `VBoxManage` as ground truth, e.g. every page of the guest's
RAM ranges (`VBoxManage debugvm <vm> info phys verbose`) must read while the VM is paused, identical to a
`VBoxManage debugvm <vm> dumpvmcore` core file, and a random pattern written to RAM pages must be
in a fresh core and survive a resume/re-pause cycle. The generic `get_vcpu_count` test expects the VM's
configured `cpus`.

**Requirements**
- a VirtualBox build with the MachineDebugger patches (MachineDebugger method) or with VBoxVMI
  (VBoxVMI method), its kernel modules loaded
- a VM with an online snapshot (saved state), e.g. a live Alpine ISO guest at its login prompt

**Configuration** (environment variables)
- `VBOX_APP_HOME`: VirtualBox bin directory (`VBoxManage`, `VBoxHeadless`, C API library)
- `VBOX_USER_HOME`: VirtualBox configuration directory holding the VM, if not the default one
- `VBOX_TEST_VM` (default `microvmi-alpine`), `VBOX_TEST_SNAPSHOT` (default `base`)
- `VBOX_TEST_METHOD`: `machinedebugger` or `vboxvmi`; unset selects an access method compiled into
  the test binary. With `vboxvmi`, setup enables VBoxVMI on the VM and teardown restores the prior
  `VBoxInternal/VMI/Enabled` value. The driver honors `VBoxInternal/VMI/SocketPath`; otherwise it
  derives the socket from VirtualBox's user home (`VBOX_USER_HOME`, legacy `~/.VirtualBox`, or
  `$XDG_CONFIG_HOME/VirtualBox` / `~/.config/VirtualBox`).
- `VBOX_TEST_MEMORY_MB`: discard the snapshot's saved state and cold boot the guest with this much RAM
  (e.g. `5120`, to test above 4 GiB). Teardown restores the snapshot, which restores the VM's RAM size.
- `VBOX_TEST_CPUS`: same, with this many vCPUs. The multi-vCPU checks (vCPU count, pause stopping
  every vCPU) need `VBOX_TEST_CPUS=2` or more; the snapshot's guest has one vCPU. Both variables can be
  combined.
- `VBOX_TEST_SEED`: replay the write test's random pattern with the seed it printed
- `VBOX_TEST_CORE_DIR` (default: the temp dir): where core dumps go. A core is as large as the guest's RAM
  (over 5 GiB for a 5 GiB guest): avoid a RAM-backed tmpfs `/tmp` for big guests. Deleted at teardown.

The VM state between each test is handled by the following commands:
- setup: `controlvm <vm> poweroff` (if running, paused or stuck), `snapshot <vm> restore <snapshot>`
  (retried while the VM settles), (VBoxVMI) `setextradata`, `VBoxHeadless --startvm <vm>`
- teardown: `controlvm <vm> poweroff`, `snapshot <vm> restore <snapshot>`, (VBoxVMI) `setextradata`

**Execution**

Run the generic tests the method supports, or the VirtualBox ones:

~~~
MICROVMI_TEST_FILTER=virtualbox_ cargo test --features virtualbox-api --test integration
MICROVMI_TEST_FILTER=virtualbox_ VBOX_TEST_MEMORY_MB=5120 cargo test --features virtualbox-api --test integration
MICROVMI_TEST_FILTER=vcpu VBOX_TEST_CPUS=2 cargo test --features virtualbox-api --test integration
MICROVMI_TEST_FILTER=virtualbox_read_ VBOX_TEST_METHOD=vboxvmi cargo test --features virtualbox-vmi --test integration
~~~

The VBoxVMI method only reads memory so far (no pause, registers, vCPU count or writes): filter its
runs to the `virtualbox_read_` tests.

If cargo runs in a container without VirtualBox, build the test binary there and run it on the host:

~~~
cargo test --features virtualbox-api --test integration --no-run   # prints the binary path
MICROVMI_TEST_FILTER=virtualbox_ VBOX_APP_HOME=/path/to/VirtualBox/bin target/debug/deps/integration-<hash>
~~~

If the VM is shared, serialize runs with `flock -o <lockfile> <command>`: `-o` keeps the lock file
descriptor out of `VBoxHeadless` and `VBoxSVC`, which would otherwise hold the lock after the run.

**Misconfiguration errors** (`tests/virtualbox_errors.rs`, standard test harness): `init` must fail
with an error naming the cause. The VBoxVMI cases (socket missing, also through `init(None, ..)`,
protocol version mismatch, bad magic) need no VM: a fake server sends the greeting. The MachineDebugger
case starts the VM with a stock VirtualBox; it is ignored by default, run it with `VBOX_APP_HOME` set to
the stock bin directory (kill a `VBoxSVC` of another build first):

~~~
cargo test --features virtualbox-vmi --test virtualbox_errors
VBOX_APP_HOME=/path/to/stock/VirtualBox/bin cargo test --features virtualbox-api --test virtualbox_errors -- --ignored
~~~
