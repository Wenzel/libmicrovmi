# libmicrovmi

<h3 align="center">
    A cross-platform unified Virtual Machine Introspection API library
</h3>

<p align="center">
    <a href="https://github.com/Wenzel/libmicrovmi/actions?query=workflow%3ACI">
        <img src="https://github.com/Wenzel/libmicrovmi/workflows/CI/badge.svg" alt="CI"/>
    </a>
    <a href="https://crates.io/crates/microvmi">
        <img src="https://img.shields.io/crates/v/microvmi.svg" alt="crates.io"/>
    </a>
    <a href="https://deps.rs/repo/github/Wenzel/libmicrovmi">
        <img src="https://deps.rs/repo/github/Wenzel/libmicrovmi/status.svg"/>
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
    <a href="https://wenzel.github.io/libmicrovmi/">
        <img src="https://img.shields.io/badge/Online-Documentation-green?style=for-the-badge&logo=gitbook" alt="online_docs"/>
    </a>
</p>

## Table of Contents

- [Overview](#overview)
- [Getting Started](#getting-started)
- [Documentation](#documentation)
- [Maintainers](#maintainers)
- [License](#license)

## Overview

`libmicrovmi` aims to provide a cross-platform unified _Virtual Machine Introspection_ API. (See [What's VMI ?](https://wenzel.github.io/libmicrovmi/explanation/vmi_api.html))

The term micro (μ) refers to the library's simplicity as well as the letter `U`
standing for `Unified` interface.

_Virtual Machine Introspection_ has been around since [2003](https://www.ndss-symposium.org/ndss2003/virtual-machine-introspection-based-architecture-intrusion-detection/),
yet the ecosystem is still heavily fragmented and lacks standards as well as interoperability. (See [VMI Ecosystem Fragmentation](https://wenzel.github.io/libmicrovmi/explanation/vmi_ecosystem.html))

The main objective is to provide the simplest virtual machine introspection abstraction, offering a standard API to interact with
any VMI provider, with a high degree of compatibility and composability to be integrated with any high-level VMI application.

The documentation keeps track of libmicrovmi's [integration status](https://wenzel.github.io/libmicrovmi/reference/integration_status.html) for each VMI apps.

![libmicrovmi_image](https://user-images.githubusercontent.com/964610/110927584-1dfc4500-8326-11eb-9ed5-a0732296082b.png)

## Getting Started

The documentation is here to guide you, whether you are a *user* or *developer*.

User

[![User documentation](https://user-images.githubusercontent.com/964610/134169948-bf8de1df-6169-4c5a-918a-04bf71fc7c61.png)](https://wenzel.github.io/libmicrovmi/tutorial/installation.html)

- I would like to install libmicrovmi on my system
- I would like to know how to setup my VMI app with libmicrovmi
- I would like to know which drivers are available and how to initialize them

Developer

[![Developer documentation](https://user-images.githubusercontent.com/964610/134168828-85f2cf4b-1d4f-455b-af10-f0ba8c49eb05.png)](https://wenzel.github.io/libmicrovmi/developer/libmicrovmi.html)

- I am developing a memory forensic / VM introspection app, and I want an API that supports multiple hypervisors at glance
- I want to add a new driver for libmicrovmi

## Documentation

Our documentation is hosted online at [![online_docs](https://img.shields.io/badge/Online-Documentation-green)](https://wenzel.github.io/libmicrovmi/)

You can find it at `doc/` as an [`mdbook`](https://rust-lang.github.io/mdBook/) 📖

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
