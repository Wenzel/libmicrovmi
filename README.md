# libmicrovmi

[![Join the chat at https://gitter.im/libmicrovmi/community](https://badges.gitter.im/libmicrovmi/community.svg)](https://gitter.im/libmicrovmi/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)
[![standard-readme compliant](https://img.shields.io/badge/readme%20style-standard-brightgreen.svg?style=flat-square)](https://github.com/RichardLitt/standard-readme)

> A unified interface on top of hypervisor's VMI APIs

## Table of Contents

- [Overview](#overview)
- [Requirements](#requirements)
- [Maintainers](#maintainers)
- [Contributing](#contributing)
- [License](#license)

## Overview

`libmicrovmi` aims to provide a cross-platform unified API and the necessary drivers to be
compatible with the diversity of hypervisors available today.

It aims to solve the problem of `libvmi` being written in `C` and a
Linux-centered library, not yet ready to offer drivers for other platforms such
as Windows and MacOS.

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

