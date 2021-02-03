#!/usr/bin/env python3

from pathlib import Path
from setuptools import setup
from setuptools_rust import Binding, RustExtension

CUR_DIR = Path(__file__).resolve().parent

with open(str(CUR_DIR / "README.md"), "r", encoding="utf-8") as fh:
    long_description = fh.read()

with open(str(CUR_DIR / "requirements.txt")) as f:
    requirements = f.read().splitlines()

setup(
    name="microvmi",
    version="0.0.2",
    author="Mathieu Tarral",
    author_email="mathieu.tarral@protonmail.com",
    description="Python bindings for libmicrovmi",
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
