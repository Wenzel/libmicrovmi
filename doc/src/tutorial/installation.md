# Installation

This section details how to install the officially distributed version of libmicrovmi.

libmicrovmi is accessible through the following languages
- Rust
- C
- Python

## Rust - Crates.io

The crate is available on crates.io:

[![Crates.io](https://img.shields.io/crates/v/microvmi?logo=rust&style=for-the-badge)](https://crates.io/crates/microvmi)

Bundled drivers:
- Xen

## C - Debian package

A Debian package `microvmi_x.x.x_amd64.deb` is available on [Github releases](https://github.com/Wenzel/libmicrovmi/releases):

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/Wenzel/libmicrovmi?color=yellowgreen&logo=github&style=for-the-badge)](https://github.com/Wenzel/libmicrovmi/releases)

It contains both the library and the headers to get started with C development

- `/usr/lib/libmicrovmi.so`
- `/usr/include/libmicrovmi.h`

Bundled drivers:
- Xen
- KVM
- VirtualBox
- memflow

## C - Windows zip archive

A zip archive `microvmi_win32.zip` containing a Windows release of `libmicrovmi` is available on [Github releases](https://github.com/Wenzel/libmicrovmi/releases):

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/Wenzel/libmicrovmi?color=blue&logo=github&style=for-the-badge)](https://github.com/Wenzel/libmicrovmi/releases)

It contains both the library and the headers to get started with C development

- `microvmi.dll`
- `libmicrovmi.h`

Bundled drivers:
- Virtualbox
- memflow

## Python - PyPI

libmicrovmi is exposing a Python interface through a native extension.

The package `microvmi` is available on [PyPI](https://pypi.org/project/microvmi/):

[![PyPI](https://img.shields.io/pypi/v/microvmi?color=blue&logo=pypi&logoColor=white&style=for-the-badge)](https://pypi.org/project/microvmi/)

create a virtualenv and install the `microvmi` package
~~~
$ virtualenv -p python3 venv
(venv) $ pip install microvmi
~~~

Note: this extension is completely independant from any existing `libmicrovmi.so` installation on you system.

Note2: the native extension has been compiled for **Linux only**.

Bundled drivers:
- Xen
- KVM
- VirtualBox
- memflow
