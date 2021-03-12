# libmicrovmi

<h3 align="center">
    A cross-platform unified interface on top of hypervisor's VMI APIs
</h3>

<p align="center">
    <a href="https://github.com/Wenzel/libmicrovmi/actions?query=workflow%3ACI">
        <img src="https://github.com/Wenzel/libmicrovmi/workflows/CI/badge.svg" al="CI"/>
    </a>
    <a href="https://crates.io/crates/microvmi">
        <img src="https://img.shields.io/crates/v/microvmi.svg" alt="crates.io"/>
    </a>
    <a href="https://docs.rs/microvmi">
        <img src="https://docs.rs/microvmi/badge.svg" alt="docs.rs">
    </a>
    <a href="https://gitter.im/libmicrovmi/community">
        <img src="https://badges.gitter.im/libmicrovmi/community.svg" alt="gitter">
    </a>
    <a href="https://gitpod.io/#https://github.com/Wenzel/libmicrovmi">
        <img src="https://img.shields.io/badge/Gitpod-ready--to--code-blue?logo=gitpod" alt="gitpod"/>
    </a>
</p>
<p align="center">
    <a href="https://libmicrovmi.github.io/">
        <img src="https://img.shields.io/badge/Online-Documentation-green?style=for-the-badge&logo=gitbook" alt="online_docs"/>
    </a>

</p>

## Table of Contents

- [Overview](#overview)
- [Documentation](#documentation)
- [Maintainers](#maintainers)
- [License](#license)

## Overview

`libmicrovmi` simply aims to provide a cross-platform unified _Virtual Machine Introspection_ API and the necessary drivers to be
compatible with the diversity of hypervisors available today.

The term micro (μ) refers to the library's simplicity as well as the letter `U`
standing for `Unified` interface.

The grand goal is to be the foundation for a VMI abstraction library that will
be
- multi-hypervisor
- multi-emulator
- cross-plaform
- high-level API
    - OS-level semantics
    - stealth breakpoints
    - virtual address translation

![libmicrovmi_image](https://user-images.githubusercontent.com/964610/67619627-51036e80-f7ed-11e9-80f6-2eb15b018108.png)

### Virtual Machine Introspection Apps

- Debugging
    - [pyvmidbg](https://github.com/Wenzel/pyvmidbg)
    - [icebox](https://github.com/thalium/icebox)
    - [rVMI](https://github.com/fireeye/rvmi)
    - [LiveCloudKd](https://github.com/comaeio/LiveCloudKd)
- Dynamic Analysis / Sandboxing
    - [DRAKVUF](https://github.com/tklengyel/drakvuf)
    - [PyREBox](https://github.com/Cisco-Talos/pyrebox)
    - [PANDA](https://github.com/panda-re/panda)
    - [DECAF](https://github.com/decaf-project/DECAF)
- Live Memory Analysis
    - [Volatility](https://github.com/volatilityfoundation/volatility) with the [`VMIAddressSpace`](https://github.com/libvmi/python/blob/d50eca447c4b3ea2ba49df847bfb7a3d6f000bc0/volatility/vmi.py)
    - [Rekall](https://github.com/google/rekall) with the [`VMIAddressSpace`](https://github.com/google/rekall/blob/e2424fb0cfd34db954101375a58fdfafeac3d2fa/rekall-core/rekall/plugins/addrspaces/vmi.py)
- OS Hardening
- Cloud Monitoring
- Fuzzing
    - [applepie](https://github.com/gamozolabs/applepie)

### Drivers

- [x] Xen
- [x] KVM (based on [KVM-VMI](https://github.com/KVM-VMI/kvm-vmi))
- [x] VirtualBox (based on [icebox](https://github.com/thalium/icebox))
- [ ] Hyper-V (based on [LiveCloudKd](https://github.com/comaeio/LiveCloudKd))
- [ ] QEMU (based on [TCG Plugins](https://github.com/comaeio/LiveCloudKd))

## Documentation

Our documentation is available in `doc/` as an [`mdbook`](https://rust-lang.github.io/mdBook/) 📖

[![online_docs](https://img.shields.io/badge/Online-Documentation-green)](https://libmicrovmi.github.io/)

To build the docs locally:
~~~
$ cargo install mdbook
$ mdbook build doc
$ xdg-open doc/book/index.html
~~~


## Maintainers

- [@Wenzel](https://github.com/Wenzel)
- [@rageagainsthepc](https://github.com/rageagainsthepc)

## License

[GNU General Public License v3.0](https://github.com/Wenzel/pyvmidbg/blob/master/LICENSE)
