# Installation

libmicrovmi can be accessed through various languages:
- Rust
- C
- Python

It comes bundled by default with the following Linux drivers:
- Xen
- KVM
- VirtualBox

# Rust - Crates.io

[![image](https://img.shields.io/crates/v/microvmi.svg)](https://crates.io/crates/microvmi)

# C - Debian package

A Debian package is available on [Github releases](https://github.com/Wenzel/libmicrovmi/releases) and contains

- `/usr/lib/libmicrovmi.so` : the C library
- `/usr/include/libmicrovmi.h` : the C development headers

# Python - PyPI

libmicrovmi is exposing a Python interface through a native extension.

The package `microvmi` is available on [PyPI](https://pypi.org/project/microvmi/):

![PyPI](https://img.shields.io/pypi/v/microvmi?style=for-the-badge)

Note: this extension is completely independant from any existing `libmicrovmi.so` installation on you system. Hence you don't have to install the debian package above.
