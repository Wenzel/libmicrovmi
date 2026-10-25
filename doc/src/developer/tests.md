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
- a VM with an online snapshot (saved state), e.g. a live Alpine ISO guest at its login prompt. The
  harness VM `microvmi-alpine` has three: `base` (1 vCPU, the default), `smp2` (2 vCPUs) and
  `smp4` (4 vCPUs), all children of `base`, saved with `VBoxInternal/VMI/HiddenPages` 16.
  To make one: restore `base`, `discardstate`, `modifyvm --cpus <n>`, start it, wait for the login
  prompt, `controlvm savestate`, `snapshot take <name>`, restore `base`.

**Configuration** (environment variables)
- `VBOX_APP_HOME`: VirtualBox bin directory (`VBoxManage`, `VBoxHeadless`, C API library)
- `VBOX_USER_HOME`: VirtualBox configuration directory holding the VM, if not the default one
- `VBOX_TEST_VM` (default `microvmi-alpine`), `VBOX_TEST_SNAPSHOT` (default `base`; `smp2`/`smp4`
  for the multi-vCPU runs)
- `VBOX_TEST_METHOD`: `machinedebugger` or `vboxvmi`, or the driver's default access method with
  VBoxVMI enabled (`default`) or not (`default-fallback`); unset selects an access method compiled
  into the test binary. With `vboxvmi` and `default`, setup enables VBoxVMI on the VM, with the
  others it unsets it; teardown restores the prior `VBoxInternal/VMI/Enabled` value. The driver honors
  `VBoxInternal/VMI/SocketPath`; otherwise it derives the socket from VirtualBox's user home
  (`VBOX_USER_HOME`, legacy `~/.VirtualBox`, or `$XDG_CONFIG_HOME/VirtualBox` / `~/.config/VirtualBox`).
- `VBOX_TEST_HIDDEN_PAGES` (default 16): `VBoxInternal/VMI/HiddenPages`, set by setup for every
  method and restored by teardown. It moves the RAM layout (the pool sits right above RAM), so it
  must be the value the snapshot was saved with.
- `VBOX_TEST_MEMORY_MB`: discard the snapshot's saved state and cold boot the guest with this much RAM
  (e.g. `5120`, to test above 4 GiB). Teardown restores the snapshot, which restores the VM's RAM size.
- `VBOX_TEST_CPUS`: same, with this many vCPUs (the guest is still booting while the test runs: use
  it for the memory and register tests, the `smp2`/`smp4` snapshots for the others). Both variables
  can be combined.
- `VBOX_TEST_SEED`: replay the write test's random pattern with the seed it printed
- `VBOX_TEST_CORE_DIR` (default: the temp dir): where core dumps go. A core is as large as the guest's RAM
  (over 5 GiB for a 5 GiB guest): avoid a RAM-backed tmpfs `/tmp` for big guests. Deleted at teardown.
- `VBOX_TEST_GUEST_SSH`: an ssh command line into the guest (split on whitespace, the guest
  command appended), for the `vboxvmi_ssh_events_` tests, whose CR0/CR4 writes the idle Alpine
  guest does not make: run them on the Ubuntu guest, whose user has passwordless `sudo`.
  They are excluded from an unfiltered Alpine suite; select them explicitly with
  `MICROVMI_TEST_FILTER=vboxvmi_ssh_events_` and the Ubuntu settings below.

The VM state between each test is handled by the following commands:
- setup: `controlvm <vm> poweroff` (if running, paused, stuck or in a guru meditation),
  `snapshot <vm> restore <snapshot>` (retried while the VM settles), `setextradata`,
  `VBoxHeadless --startvm <vm>`
- teardown: `controlvm <vm> poweroff`, `snapshot <vm> restore <snapshot>`, `setextradata`

**Execution**

The per-test time limit is 60 s plus 10 s per GiB of guest RAM; an unfiltered run takes about 25
minutes. Run the whole suite under each access method, then the VBoxVMI suite on the 2- and
4-vCPU snapshots, then the tests that need another guest or VM size:

~~~
export VBOX_APP_HOME=/path/to/VirtualBox/bin VBOX_USER_HOME=/path/to/vbox-home
VBOX_TEST_METHOD=machinedebugger cargo test --features virtualbox --test integration
for s in base smp2 smp4; do VBOX_TEST_SNAPSHOT=$s VBOX_TEST_METHOD=vboxvmi cargo test --features virtualbox --test integration; done
MICROVMI_TEST_FILTER=virtualbox_ VBOX_TEST_MEMORY_MB=5120 cargo test --features virtualbox-api --test integration
MICROVMI_TEST_FILTER=vboxvmi_ssh_events_ VBOX_TEST_VM=microvmi-ubuntu VBOX_TEST_HIDDEN_PAGES=256 VBOX_TEST_GUEST_SSH='ssh -i /path/to/id_ed25519 -p 2222 -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no -o BatchMode=yes vmi@127.0.0.1' VBOX_TEST_METHOD=vboxvmi cargo test --features virtualbox-vmi --test integration
~~~

Under `machinedebugger` the method has no events: the VBoxVMI-only tests (`vboxvmi_*`) and the
generic `intercept_cr3_*` tests fail, so filter that run with `MICROVMI_TEST_FILTER=virtualbox_` and
the generic test names. `virtualbox_read_physical_matches_core` also fails there while
`HiddenPages` is set: the MachineDebugger method counts the Hidden page pool as RAM
([Wenzel/drakvuf#32](https://github.com/Wenzel/drakvuf/issues/32)). Every test
leaves the VM as the snapshot was, so a failed run can be resumed with a filter.

`virtualbox_write_physical_` checks random patterns against a fresh core dump, after
resume/re-pause and after restoring the original pages. `vboxvmi_disconnect_` covers normal close,
a killed client process, a killed client with a vCPU parked at a single-step, CR3 write, MSR write
or page-access event (and no page access restriction left for the next client), both
external-pause ownership cases, a repeated external resume/pause versus disconnect race, and
immediate `Busy` refusal of a second client. `vboxvmi_reset_ends_session` and
`vboxvmi_save_state_refused_while_connected` start from a session holding every kind of state (a
view with a Remap that every vCPU runs in, the whole Hidden page pool, a restriction, Intercepts on
every vCPU and vCPU 0 parked at a single step): a reset ends it and the next client finds none of
it; saving the state and a live snapshot are refused and the session goes on.

`vboxvmi_events_` checks the int3 at RIP (translated via the guest's page tables),
single-step (never into an interrupt handler), parked-vCPU register access, pause/resume, invalid
replies, and the VirtualBox debugger exclusion both ways with a breakpoint and a CR3 Intercept
through the debugger console (the VBoxVMI context enables it on TCP port 5000, which must be
free). Its CR and MSR write tests (VT-x hosts) check events against the parked vCPU's registers
and the `MOV CRx`/`WRMSR` bytes at RIP, refused reinjection, no events after clearing, Intercepts
set and cleared while paused, a reply while paused, and the CR event before the single-step event;
on several vCPUs the CR0/CR4 tests also check that the vCPU without the Intercept gives no event.
The CR3 test sets its Intercept on every vCPU (the guest's scheduler picks the vCPU) and types
`ls /` at the guest console to start processes; the CPUID test logs in as root and reads
`/dev/cpu/0/cpuid` (Alpine's musl runs no CPUID); `vboxvmi_ssh_events_` gets CR4 writes
(including CR4.TSD-only flips, a bit VirtualBox leaves to the guest) and CR0.CD writes from
commands run over `VBOX_TEST_GUEST_SSH`. The page-access tests (VT-x hosts) restrict pages the
idle guest touches, found while paused: vCPU 0's RIP page (the idle loop, executed at every timer
interrupt) for execute events, and its RSP page (the idle stack) for write and read events. They
check gpa/gva against the page tables and RIP, `get_page_access` round trips, refusals (write
without read, the local APIC MMIO page, reinjection, an emulation reply that does not fit the
event), retry faulting again, `Emulate` stepping past the instruction with the restriction kept,
`EmulateReadData` landing in the register of a `POP` from the stack (then restored),
`EmulateInsn` at an int3 running the original bytes, the page-access event before the
single-step event, the debugger exclusion for a restriction, and 3 s of restricting and lifting
both pages in the Default view and in a view every vCPU runs in, with the Intercept on every vCPU
(`vboxvmi_events_page_access_high_rate`).

An int3 written at vCPU 0's RIP (the idle loop) runs on every vCPU: the breakpoint tests set the
Breakpoint Intercept on every vCPU, take the first vCPU that stops at it, and release the others
once the original byte is back. The emulated int3 test restricts that page on every vCPU, has the
int3 emulated for the first vCPU that faults at it and passes the others' instructions on the page
(`Emulate`, or `UnrestrictedStep` where emulation is refused). A vCPU counts as running again when
it executes within 5 s (an idle vCPU may halt for over a second). `vboxvmi_memory_view_` remaps the
idle loop's page to Hidden page copies: an int3 reported in the view and retried in the Default
view, view numbering and the 15-view limit, no destroying the Default view or a view some vCPU
runs in (after a switch, or after
an event reply moved one vCPU), two views with different copies, a restored Remap (also while a vCPU
is parked at it), and every vCPU stopping at the int3 after 200 Remap changes while they run.
`vboxvmi_hidden_pool_exhaustion` allocates the whole pool (distinct pages above the guest's memory,
each read back with its own pattern), checks the next allocation, double and RAM-page frees fail and
that a freed page is handed out again.

The other generic filters cover `pause`, `double_pause`, `pause_resume`, `multiple_pause_resume`,
`virtualbox_pause_*` and `virtualbox_registers_unchanged_while_paused`; `vcpu_count` covers
`get_vcpu_count` and `virtualbox_vcpu_count_matches_vmm`. On several vCPUs, the register tests read
every vCPU and write the last one.

`virtualbox_default_method_selection` (`virtualbox` feature) checks the default access method under
any `VBOX_TEST_METHOD`: with VBoxVMI enabled, the context's driver holds the socket and a second
default init must fail as busy instead of falling back; with it disabled, the socket is missing, the
driver (MachineDebugger) reads memory, and an explicit VBoxVMI without a socket path must fail. Run it
under each selection:

~~~
for m in default default-fallback vboxvmi machinedebugger; do MICROVMI_TEST_FILTER=virtualbox_default_method_selection VBOX_TEST_METHOD=$m RUST_LOG=microvmi=info cargo test --features virtualbox --test integration; done
~~~

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
