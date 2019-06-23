# libmicrovmi

[![Join the chat at https://gitter.im/libmicrovmi/community](https://badges.gitter.im/libmicrovmi/community.svg)](https://gitter.im/libmicrovmi/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)
[![standard-readme compliant](https://img.shields.io/badge/readme%20style-standard-brightgreen.svg?style=flat-square)](https://github.com/RichardLitt/standard-readme)

> A cross-platform unified interface on top of hypervisor's VMI APIs

## Table of Contents

- [Overview](#overview)
- [Requirements](#requirements)
- [Maintainers](#maintainers)
- [Contributing](#contributing)
- [License](#license)

## Overview

`libmicrovmi` simply aims to provide a cross-platform unified API and the necessary drivers to be
compatible with the diversity of hypervisors available today.

Its main goal is to solve the problem of [`libvmi`](https://github.com/libvmi/libvmi) being written in `C` and a
Linux-centered library, not yet ready to offer drivers for other platforms such
as Windows and MacOS.

It doesn't aim to **replace** `libvmi`, since it doesn't implement features like rekall profile parsing or operating system knowledge, but rather to provide a foundation for high-level VMI libraries.

The term micro (μ) refers to the library's simplicity as well as the letter `U`
standing for `Unified` interface.

![libmicrovmi_image](https://user-images.githubusercontent.com/964610/58368164-bec30b80-7ed8-11e9-8a39-c85257cfbe38.png)

## VMI API

* Query and modify the VM hardware state
    - read/write VCPU registers
    - read/write physical memory
* Subscribe and listen to hardware events
    - mov to/from CR3/CR8
    - mov to/from DRx
    - mov to/from MSR
    - interrupts
    - singlestep (MTF)
    - SLAT (Second Level Address Translation) events
        - `r/w/x` event on a page
        - dynamically switch to multiple memory _views_ using alternate SLAT pointers
- Utilities
    - pagefault injection

## Requirements

- `cargo`

## References

- [LibVMI C library](https://github.com/libvmi/libvmi): Simplified Virtual Machine Introspection

## Maintainers

[@Wenzel](https://github.com/Wenzel)

## Contributing

PRs accepted.

Small note: If editing the Readme, please conform to the [standard-readme](https://github.com/RichardLitt/standard-readme) specification.

## License

[GNU General Public License v3.0](https://github.com/Wenzel/pyvmidbg/blob/master/LICENSE)

