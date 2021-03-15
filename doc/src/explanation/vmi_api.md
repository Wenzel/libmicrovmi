# VMI API

This section describes what can be done with a _virtual machine introspection_ API

* Query and modify the VM hardware state
    - read/write VCPU registers
    - read/write physical memory
* Subscribe and listen to hardware events
    - mov to/from CR3/CR8
    - mov to/from DRx
    - mov to/from MSR
    - interrupts
    - singlestep (MTF)
    - hypercalls
    - descriptors
    - SLAT (Second Level Address Translation) events
        - `r/w/x` event on a page
        - dynamically switch to multiple memory views using alternate SLAT pointers
    - Intel Processor Trace packets
- Utilities
    - foreign mapping
    - pagefault injection


# Xen

VMI APIs are available upstream since Xen 4.1

- [Xen Wiki on Virtual Machine Introspection](https://wiki.xenproject.org/wiki/Virtual_Machine_Introspection)
- [VMI APIs can be found in `xenctrl.h`](https://github.com/xen-project/xen/blob/RELEASE-4.12.0/tools/libxc/include/xenctrl.h)

- Accessing the VM's hardware state
  - get number of VCPUs: ✅
  - get maximum gfn: ✅
  - pause/resume: ✅
  - r/w physical memory: ✅
  - r/w virtual memory: ✅
  - r/w VCPU registers: ✅
- Intercept VM's hardware events
  - control registers: ✅
  - extended control registers: ✅
  - debug registers: ✅
  - MSR: ✅
  - singlesteps: ✅
  - interrupts: ✅
  - descriptors: ✅
  - hypercalls: ✅
  - CPUID: ✅
  - memory: ✅
  - alternate SLAT: ✅
- Utilities:
  - foreign mapping: ✅
  - exception injection: ✅

# KVM

VMI APIs are currently being developed by `BitDefender`, and in review on the mailing list.

- [KVM-VMI project](https://github.com/KVM-VMI/kvm-vmi)
- [`libkvmi.h`](https://github.com/KVM-VMI/kvm/blob/528c2680bec46e9603126eec6506bc5da71d297b/tools/kvm/kvmi/include/kvmi/libkvmi.h)
- [`kvmi.h`](https://github.com/KVM-VMI/kvm/blob/528c2680bec46e9603126eec6506bc5da71d297b/arch/x86/include/uapi/asm/kvmi.h)

- Accessing the VM's hardware state
  - get number of VCPUs: ✅
  - get maximum gfn: ❌
  - pause/resume: ✅
  - r/w physical memory: ✅
  - r/w virtual memory: ❌
  - r/w VCPU registers: ✅
- Intercept VM's hardware events
  - control registers: ✅
  - extended control registers: ❌
  - debug registers: ✅
  - MSR: ✅
  - singlesteps: ❌
  - interrupts: ✅
  - descriptors: ✅
  - hypercalls: ✅
  - CPUID: ❌
  - memory: ✅
  - alternate SLAT: ❌
- Utilities:
  - foreign mapping: ✅
  - exception injection: ✅

Note:
* `SLAT`: _Second Level Address Translation_
