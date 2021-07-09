import subprocess
from pathlib import Path

import nox

CUR_DIR = Path(__file__).parent

# default sessions for nox
nox.options.sessions = ["fmt", "lint", "type", "test"]


@nox.session
def fmt(session):
    session.install("black==20.8b1")
    # note: black doesn't support setup.cfg
    # so we hardcode the config here
    session.run("black", "--line-length", "120", ".")


@nox.session
def lint(session):
    session.install("flake8", "flake8-bugbear", "isort")
    session.run("flake8", "--show-source", "--statistics")
    session.run("isort", "--line-length", "120", ".")


@nox.session
def type(session):
    session.install("-r", "requirements.txt")
    session.install("mypy")
    session.run("mypy", "-p", "microvmi")


@nox.session
def test(session):
    # run unit tests
    args = session.posargs
    # we need to compile and install the extension
    session.install("-r", "requirements.txt")
    # can't use pip install
    # see: https://github.com/PyO3/maturin/issues/330
    session.run(f'{CUR_DIR / "setup.py"}', "develop")
    session.install("pytest==6.0.2", "coverage==5.3")
    session.run("coverage", "run", "-m", "pytest", "-v", "-k", "unit", *args)
    session.run("coverage", "report")


@nox.session
def test_volatility_xen(session):
    """Run the PsList volatility plugin on the Xen domain specified by the URL"""
    # example:
    # nox -r -s test_volatility_xen -- <url>
    args = session.posargs
    if not args:
        raise RuntimeError("URL required. Example: nox -r -s test_volatility_xen -- vmi:///win10")
    # we need to compile and install the extension
    session.install("-r", "requirements.txt")
    # make sure we have volatility
    # Note: we need to use the latest unreleased dev code from Github
    session.install("git+https://github.com/volatilityfoundation/volatility3@af090bf29e6bb26a5961e0a6c25b5d1ec6e82498")
    # can't use pip install
    # see: https://github.com/PyO3/maturin/issues/330
    session.run(f'{CUR_DIR / "setup.py"}', "develop", "--features", "xen")
    vol_path = Path(__file__).parent / ".nox" / "test_volatility_xen" / "bin" / "vol"
    plugins_dir = Path(__file__).parent / "microvmi" / "volatility"
    session.run(
        "sudo",
        "-E",
        str(vol_path),
        "--plugin-dirs",
        str(plugins_dir),
        "--single-location",
        *args,
        "windows.pslist.PsList",
    )


@nox.session
def test_volatility_kvm(session):
    """Run the PsList volatility plugin on the KVM domain specified by the URL"""
    # example:
    # nox -r -s test_volatility_kvm -- <url>
    args = session.posargs
    if not args:
        raise RuntimeError(
            "URL required. Example: nox -r -s test_volatility_kvm -- vmi:///win10?kvmi_unix_socket=/tmp/introspector"
        )
    # we need to compile and install the extension
    session.install("-r", "requirements.txt")
    # make sure we have volatility
    # Note: we need to use the latest unreleased dev code from Github
    session.install("git+https://github.com/volatilityfoundation/volatility3@af090bf29e6bb26a5961e0a6c25b5d1ec6e82498")
    # can't use pip install
    # see: https://github.com/PyO3/maturin/issues/330
    session.run(f'{CUR_DIR / "setup.py"}', "develop", "--features", "kvm")
    vol_path = Path(__file__).parent / ".nox" / "test_volatility_kvm" / "bin" / "vol"
    plugins_dir = Path(__file__).parent / "microvmi" / "volatility"
    session.run(
        str(vol_path),
        "--plugin-dirs",
        str(plugins_dir),
        "--single-location",
        *args,
        "windows.pslist.PsList",
    )


@nox.session
def test_volatility_memflow(session):
    """Run the PsList volatility plugin on the memflow connector specified by the URL"""
    # example:
    # nox -r -s test_volatility_memflow -- vmi:///?memflow_connector_name=qemu_procfs
    args = session.posargs
    if not args:
        raise RuntimeError("URL required. Example: nox -r -s test_volatility_memflow -- vmi:///...")
    # we need to compile and install the extension
    session.install("-r", "requirements.txt")
    # make sure we have volatility
    # Note: we need to use the latest unreleased dev code from Github
    session.install("git+https://github.com/volatilityfoundation/volatility3@af090bf29e6bb26a5961e0a6c25b5d1ec6e82498")
    # can't use pip install
    # see: https://github.com/PyO3/maturin/issues/330
    session.run(f'{CUR_DIR / "setup.py"}', "develop", "--features", "mflow")
    vol_path = Path(__file__).parent / ".nox" / "test_volatility_memflow" / "bin" / "vol"
    plugins_dir = Path(__file__).parent / "microvmi" / "volatility"
    session.run(
        "sudo",
        "-E",
        str(vol_path),
        "--plugin-dirs",
        str(plugins_dir),
        "--single-location",
        *args,
        "windows.pslist.PsList",
    )


@nox.session
def coverage_html(session):
    session.install("coverage==5.3")
    session.run("coverage", "html", "--dir", ".coverage_html")
    session.run("xdg-open", ".coverage_html/index.html")


@nox.session
def generate_wheels(session):
    # you can pass additional argument
    # example:
    #   nox -r -s generate_wheels -- --features xen
    #   nox -r -s generate_wheels -- --features xen,kvm,virtualbox --release
    args = session.posargs
    image_name = "manylinux2014_microvmi"
    # ensure custom image is up to date
    subprocess.check_call(["docker", "build", "-t", image_name, str(CUR_DIR)])
    root_dir = CUR_DIR.parent
    # maps libmicrovmi root dir to container /io
    # executes /io/python/build-wheels.sh
    subprocess.check_call(
        ["docker", "run", "--rm", "-v", f"{str(root_dir)}:/io", image_name, "/io/python/build-wheels.sh", *args]
    )
