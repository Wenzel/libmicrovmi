import pytest


def pytest_addoption(parser):
    """add a new option to pass a domain name"""
    parser.addoption("--domain", action="store", help="pass a domain / VM name to be used for the tests (volatility)")


@pytest.fixture
def arg_domain_name(pytestconfig):
    return pytestconfig.getoption("domain")
