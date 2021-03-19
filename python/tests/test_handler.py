import pytest
from microvmi import DriverType
from microvmi.handler import MicrovmiHandlerError, url_to_driver_parameters


def test_valid_scheme():
    url = "vmi:///vm_name"
    url_to_driver_parameters(url)


def test_invalid_scheme():
    url = "http:///vm_name"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_empty_vm_name():
    url = "vmi:///"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_vm_name():
    expected = "windows10"
    url = f"vmi:///{expected}"
    value, *rest = url_to_driver_parameters(url)
    assert expected == value


def test_parse_vm_name_spaces():
    expected = "windows 10 20H1"
    url = f"vmi:///{expected}"
    value, *rest = url_to_driver_parameters(url)
    assert expected == value


def test_parse_hypervisor_none():
    expected = None
    url = "vmi:///vm_name"
    _, value, _ = url_to_driver_parameters(url)
    assert expected == value


@pytest.mark.parametrize("drv_type_enum_member", [d for d in DriverType])
def test_parse_hypervisor_each(drv_type_enum_member):
    expected = drv_type_enum_member
    url = f"vmi://{drv_type_enum_member.name}/vm_name"
    _, value, _ = url_to_driver_parameters(url)
    assert expected == value


def test_parse_hypervisor_bad():
    url = "vmi://UNKNOWN_DRIVER/vm_name"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_init_param_none():
    expected = None
    url = "vmi:///vm_name"
    _, _, value = url_to_driver_parameters(url)
    assert expected == value


def test_parse_init_param_kvmi_socket():
    socket = "/tmp/introspector"
    # TODO: missing getter to test init_param type
    url = f"vmi:///vm_name?kvmi_unix_socket={socket}"
    _, _, value = url_to_driver_parameters(url)
    assert socket == value.param_data_string


def test_parse_init_param_unkown_key():
    key = "unkown_config_key"
    socket = "/tmp/introspector"
    url = f"vmi:///vm_name?{key}={socket}"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)
