# installation

This section details how to install the officially distributed version of libmicrovmi.

libmicrovmi is accessible through the following languages
- Rust
- C
- Python

It comes bundled by default with the following Linux drivers:
- Xen
- KVM
- VirtualBox
- memflow

# Rust - Crates.io

The crate is available on crates.io:

[![Crates.io](https://img.shields.io/crates/v/microvmi?logo=rust&style=for-the-badge)](https://crates.io/crates/microvmi)

# C - Debian package

A Debian package is available on [Github releases](https://github.com/Wenzel/libmicrovmi/releases):

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/Wenzel/libmicrovmi?color=yellowgreen&logo=github&style=for-the-badge)](https://github.com/Wenzel/libmicrovmi/releases)

It contains both the library and the headers to get started with C development

- `/usr/lib/libmicrovmi.so`
- `/usr/include/libmicrovmi.h`

# Python - PyPI

libmicrovmi is exposing a Python interface through a native extension.

The package `microvmi` is available on [PyPI](https://pypi.org/project/microvmi/):

[![PyPI](https://img.shields.io/pypi/v/microvmi?color=blue&logo=pypi&logoColor=white&style=for-the-badge)](https://pypi.org/project/microvmi/)

create a virtualenv and install the `microvmi` package
~~~
$ virtualenv -p python3 venv
(venv) $ pip install microvmi
~~~

Note: this extension is completely independant from any existing `libmicrovmi.so` installation on you system. Hence you don't have to install the debian package above.
