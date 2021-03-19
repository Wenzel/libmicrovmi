#!/usr/bin/env python3

import sys
from pathlib import Path

import toml
from setuptools import setup
from setuptools_rust import Binding, RustExtension


CUR_DIR = Path(__file__).resolve().parent

# check for --features
try:
    index = sys.argv.index("--features")
except ValueError:
    features = []
else:
    # remove --features
    sys.argv.pop(index)
    # remove and retieve --features arg
    try:
        features_str = sys.argv.pop(index)
    except IndexError:
        print("invalid --features argument")
        sys.exit(1)
    else:
        features = features_str.split(",")

# check for --release
debug = False
try:
    index = sys.argv.index("--release")
except ValueError:
    debug = True
else:
    debug = False
    sys.argv.pop(index)

# python package version is taken from Cargo.toml to avoid duplication
with open(str(CUR_DIR / "Cargo.toml"), "r", encoding="utf-8") as f:
    cargo = f.read()
    cargo_toml = toml.loads(cargo)
    author = cargo_toml["package"]["authors"][0].split("<")[0].strip()
    author_email = cargo_toml["package"]["authors"][0].split("<")[1][:-1].strip()

with open(str(CUR_DIR / cargo_toml["package"]["readme"]), "r", encoding="utf-8") as fh:
    long_description = fh.read()

with open(str(CUR_DIR / "requirements.txt")) as f:
    requirements = f.read().splitlines()

setup(
    name="microvmi",
    version=cargo_toml["package"]["version"],
    author=author,
    author_email=author_email,
    description=cargo_toml["package"]["description"],
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/Wenzel/libmicrovmi",
    install_requires=requirements,
    rust_extensions=[RustExtension("microvmi.pymicrovmi", binding=Binding.PyO3, features=features, debug=debug)],
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
