# Hidden pages are a pool reserved at VM creation

**Remap** targets must be RAM the guest is never told about. PGM registers RAM and MMIO2 ranges
only while the VM is being created (or loaded), and saved state expects the same ranges back, so
the patched VirtualBox reserves `VBoxInternal/VMI/HiddenPages` pages (default 0) right above the
highest **RAM range** when VBoxVMI is enabled. They are not listed in the **RAM ranges**; the client
takes and returns them one at a time, and allocation fails once the pool is empty.

## Considered options

- Adding RAM at runtime, as Xen's `populate_physmap` does: needs RAM-range registration while the
  VM runs, which PGM does not support and saved state would not survive.
