import nox

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
