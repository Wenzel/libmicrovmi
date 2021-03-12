# libmicrovmi

<h3 align="center">
    A cross-platform unified Virtual Machine Introspection API library
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

`libmicrovmi` aims to provide a cross-platform unified _Virtual Machine Introspection_ API.

The term micro (μ) refers to the library's simplicity as well as the letter `U`
standing for `Unified` interface.

_Virtual Machine Introspection_ has been around since [2003](https://www.ndss-symposium.org/ndss2003/virtual-machine-introspection-based-architecture-intrusion-detection/),
yet the ecosystem is still heavily fragmented and lacks standards as well as interoperability.

See [Documentation: VMI Ecosystem Fragmentation](https://libmicrovmi.github.io/explanation/vmi_ecosystem.html)

The main objective is to provide the simplest virtual machine introspection abstraction, offering a standard API to interact with
any VMI provider.

![libmicrovmi_image](https://user-images.githubusercontent.com/964610/110927584-1dfc4500-8326-11eb-9ed5-a0732296082b.png)

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
