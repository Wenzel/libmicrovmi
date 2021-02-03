#!/usr/bin/env python3
from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="microvmi",
    version="0.0.1",
    rust_extensions=[RustExtension("microvmi.pymicrovmi", binding=Binding.PyO3)],
    packages=["microvmi"],
    # rust extensions are not zip safe, just like C-extensions.
    zip_safe=False,
)
