import subprocess
from pathlib import Path

import nox

CUR_DIR = Path(__file__).parent

# default sessions for nox
nox.options.sessions = ["fmt", "lint", "type"]


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
    session.install("-r", "requirements.txt")
    session.install("pytest==6.0.2", "coverage==5.3")
    session.run("coverage", "run", "-m", "pytest", "-v", *args)
    session.run("coverage", "report")


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
