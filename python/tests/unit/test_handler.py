import pytest
from microvmi import DriverType, VBoxInitParamsPy
from microvmi.volatility.vmi_handler import MicrovmiHandlerError, url_to_driver_parameters


def test_valid_scheme():
    url = "vmi:///"
    url_to_driver_parameters(url)


def test_invalid_scheme():
    url = "http:///"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_vm_name():
    expected = "windows10"
    url = f"vmi:///?vm_name={expected}"
    *rest, init_params = url_to_driver_parameters(url)
    assert expected == init_params.common.vm_name


def test_parse_vm_name_spaces():
    expected = "windows 10 20H1"
    url = f"vmi:///?vm_name={expected}"
    *rest, init_params = url_to_driver_parameters(url)
    assert expected == init_params.common.vm_name


def test_parse_hypervisor_none():
    expected = None
    url = "vmi:///vm_name"
    drv_type, _ = url_to_driver_parameters(url)
    assert expected == drv_type


@pytest.mark.parametrize("drv_type_enum_member", [d for d in DriverType])
def test_parse_hypervisor_each(drv_type_enum_member):
    expected = drv_type_enum_member
    url = f"vmi://{drv_type_enum_member.name}/vm_name"
    drv_type, _ = url_to_driver_parameters(url)
    assert expected == drv_type


def test_parse_hypervisor_bad():
    url = "vmi://UNKNOWN_DRIVER/vm_name"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_init_param_kvmi_socket():
    socket = "/tmp/introspector"
    # TODO: missing getter to test init_param type
    url = f"vmi://?kvm_unix_socket={socket}"
    _, init_params = url_to_driver_parameters(url)
    assert socket == init_params.kvm.unix_socket


def test_parse_init_param_unknown_key():
    key = "unkown_config_key"
    socket = "/tmp/introspector"
    url = f"vmi://?{key}={socket}"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_init_param_vbox_access_method():
    url = "vmi://VirtualBox/?vbox_access_method=machinedebugger"
    _, init_params = url_to_driver_parameters(url)
    assert isinstance(init_params.virtualbox, VBoxInitParamsPy.MachineDebugger)


def test_parse_vbox_default_access_method():
    _, init_params = url_to_driver_parameters("vmi://VirtualBox/?vm_name=windows10")
    assert init_params.virtualbox is None


def test_parse_init_param_vbox_vboxvmi():
    url = "vmi://VirtualBox/?vbox_access_method=vboxvmi&vbox_socket_path=/tmp/vm.sock"
    _, init_params = url_to_driver_parameters(url)
    assert isinstance(init_params.virtualbox, VBoxInitParamsPy.VBoxVMI)
    assert init_params.virtualbox.socket_path == "/tmp/vm.sock"


def test_parse_init_param_vbox_vboxvmi_no_socket():
    # VBoxVMI at the VM's default socket
    url = "vmi:///?vbox_access_method=vboxvmi"
    _, init_params = url_to_driver_parameters(url)
    assert isinstance(init_params.virtualbox, VBoxInitParamsPy.VBoxVMI)
    assert init_params.virtualbox.socket_path is None


def test_parse_init_param_vbox_socket_requires_vboxvmi():
    url = "vmi:///?vbox_access_method=machinedebugger&vbox_socket_path=/tmp/vm.sock"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)


def test_parse_init_param_vbox_access_method_unknown():
    url = "vmi:///?vbox_access_method=unknown"
    with pytest.raises(MicrovmiHandlerError):
        url_to_driver_parameters(url)
