#!/usr/bin/env python3

from pathlib import Path

import toml
from setuptools import setup
from setuptools_rust import Binding, RustExtension


CUR_DIR = Path(__file__).resolve().parent

# python package version is taken from Cargo.toml to avoid duplication
with open(str(CUR_DIR / "Cargo.toml"), "r", encoding="utf-8") as f:
    cargo = f.read()
    cargo_toml = toml.loads(cargo)

with open(str(CUR_DIR / cargo_toml['package']['readme']), "r", encoding="utf-8") as fh:
    long_description = fh.read()

with open(str(CUR_DIR / "requirements.txt")) as f:
    requirements = f.read().splitlines()

setup(
    name="microvmi",
    version=cargo_toml['package']['version'],
    author="Mathieu Tarral",
    author_email="mathieu.tarral@protonmail.com",
    description=cargo_toml['package']['description'],
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/Wenzel/libmicrovmi",
    install_requires=requirements,
    rust_extensions=[RustExtension("microvmi.pymicrovmi", binding=Binding.PyO3)],
    packages=["microvmi"],
    # rust extensions are not zip safe, just like C-extensions.
    zip_safe=False,
    classifiers=[
        "Programming Language :: Python :: 3.6",
        "Development Status :: 4 - Beta",
        "License :: OSI Approved :: GNU General Public License v3 (GPLv3)",
    ],
    python_requires=">=3.6",
)
