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
- a VirtualBox build with the MachineDebugger patches (MachineDebugger method), VBoxVMI
  (VBoxVMI method), or both patch series on a test-only combined branch (cross-method test), with
  that build's kernel modules loaded
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
for f in init vcpu_count pause double_resume virtualbox_read_ virtualbox_write_physical_ registers_match write_registers vboxvmi_disconnect_; do MICROVMI_TEST_FILTER=$f MICROVMI_TEST_TIMEOUT=120 VBOX_TEST_METHOD=vboxvmi cargo test --features virtualbox-vmi --test integration; done
for f in vcpu pause registers_match write_registers; do MICROVMI_TEST_FILTER=$f VBOX_TEST_CPUS=2 VBOX_TEST_METHOD=vboxvmi cargo test --features virtualbox-vmi --test integration; done
~~~

The VBoxVMI filters above run every test, each once; `virtualbox_write_physical_` checks random
patterns against a fresh core dump, after resume/re-pause and after restoring the original pages.
`vboxvmi_disconnect_` covers normal close, a killed client process, both external-pause ownership
cases, a repeated external resume/pause versus disconnect race, and immediate `Busy` refusal of a
second client. The other filters cover `pause`,
`double_pause`, `pause_resume`, `multiple_pause_resume`, `virtualbox_pause_*` and
`virtualbox_registers_unchanged_while_paused`; `vcpu_count` covers `get_vcpu_count` and
`virtualbox_vcpu_count_matches_vmm`. With `VBOX_TEST_CPUS=2`, the register tests read every vCPU
and write the last one.

### MachineDebugger/VBoxVMI cross-method consistency

The `cross_method_consistency` inventory test opens MachineDebugger and VBoxVMI
concurrently against the same VM process, pauses it through MachineDebugger, and compares every
register on every vCPU and every byte in the live PGM RAM ranges directly between the two methods.
It checks both drivers' maximum physical address against the end of those ranges, reports the exact
byte count above 4 GiB, and uses no core dump. It then writes the same safe zero-page locations used
by the other VirtualBox write test and the last vCPU's `SYSENTER_ESP`/`SYSENTER_EIP` in both
directions. Every write is read independently through the other method. Originals are restored and
verified through both methods before resume; an unwind guard repeats restoration, and the standard
teardown powers off and restores the base snapshot.

One VirtualBox process must implement both APIs. The long-lived `vmi/machinedebugger` and
`vmi/vboxvmi` branches intentionally stay separate; use the test-only
`Wenzel/virtualbox` branch `vmi/cross-method-test`, which merges the completed MachineDebugger
patches into `vmi/vboxvmi`:

~~~
git -C /path/to/virtualbox fetch fork
git -C /path/to/virtualbox worktree add /path/to/virtualbox-vmi fork/vmi/cross-method-test
/path/to/vbox-dev/build.sh /path/to/virtualbox-vmi
~~~

Compile the integration binary with both access methods. Run it on the host with
`VBOX_APP_HOME` pointing at that same combined build: the VM's VBoxVMI server and
MachineDebugger's dynamically loaded `VBoxC.so` must come from one build.
`VBOX_TEST_METHOD=vboxvmi` enables the server and supplies its one client to the test.
`VBOX_TEST_CROSS_METHOD=1` opts into this destructive live proof; without it an unfiltered run
prints an explicit skip, and ordinary `virtualbox_` filters do not select it. The test requires
the exact `MICROVMI_TEST_FILTER=cross_method_consistency` and two vCPUs. Run both the 512 MiB case
and the 5120 MiB case, which must report nonzero bytes and a selected write page above 4 GiB:

~~~
cargo test --features 'virtualbox-api virtualbox-vmi' --test integration --no-run
BIN=target/debug/deps/integration-<hash>
COMMON='VBOX_APP_HOME=/path/to/virtualbox-vmi/out/linux.amd64/release/bin VBOX_USER_HOME=/path/to/vbox-home VBOX_TEST_METHOD=vboxvmi VBOX_TEST_CROSS_METHOD=1 VBOX_TEST_CPUS=2 MICROVMI_TEST_FILTER=cross_method_consistency MICROVMI_TEST_TIMEOUT=180 VBOX_TEST_CORE_DIR=/path/to/disk-backed/core-dir'
flock -o /path/to/vbox-dev/.vm.lock env $COMMON VBOX_TEST_MEMORY_MB=512 $BIN
flock -o /path/to/vbox-dev/.vm.lock env $COMMON VBOX_TEST_MEMORY_MB=5120 $BIN
~~~

Run only a binary built from the matching libmicrovmi checkout. Teardown runs after setup or test
failure; do not interrupt teardown. A completed run leaves the VM powered off with the base
snapshot restored.

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
