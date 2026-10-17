# Memory views are separate EPT root trees, switched by swapping a vCPU's EPT root

VBoxVMI needs several **Memory views** per VM, each a vCPU can run in. The patched VirtualBox
already keeps two EPT trees per VM for **Page access restrictions** (restricted for vCPUs with the
page-access **Intercept**, unrestricted for the others), keyed in the PGM pool and selected per vCPU
by replacing its EPT root from ring 3 on its EMT. We generalize that key from one bit to a view
number (up to 16 views, the **Default view** included, each in a restricted and an unrestricted
tree), and switch views the same way.

## Considered options

- VMFUNC EPTP switching with an EPTP list: switches in the guest without an exit, but VirtualBox
  strips `VMX_PROC_CTLS2_VMFUNC` and has no EPTP list support; it would be a new HM feature on
  top of the views themselves, for switches that in VBoxVMI only happen at an event reply anyway.

## Consequences

- A view switch outside an event reply (all vCPUs) is an EMT rendezvous.
- **Page access restrictions** no longer fit the 3 spare bits of `PGMPAGE`; they move to a per-view
  table.
- Instructions VirtualBox emulates (IEM) look memory up by guest-physical address, bypassing EPT:
  they must apply the **Active view**'s **Remaps** and restrictions themselves.
- The per-view table (`VmiPages`, sparse by guest-physical page) lives on the ring-3 process heap,
  unlike `PGMPAGE`'s bits: ring-0 and raw-mode cannot dereference it. A vCPU whose Active view is
  not the Default view therefore never builds EPT entries in ring-0; the EPT fault handler returns
  `VINF_EM_RAW_EMULATE_INSTR` immediately, and every instruction runs through IEM (ring-3 once
  ring-0 IEM also finds the page's Remap/restriction unresolvable) instead of executing directly
  in hardware. Slower, but correct and worth it: VBoxVMI Memory views are opt-in and not a hot path.
