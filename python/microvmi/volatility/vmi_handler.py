import logging
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import parse_qs, urlparse
from urllib.request import BaseHandler, Request

from microvmi import CommonInitParamsPy, DriverInitParamsPy, DriverType, KVMInitParamsPy, MemflowInitParamsPy, Microvmi

# to be used by volatility, the VMIHandler should inherit from VolatilityHandler
# in order to be non cacheable
# if we find volatility, use VolatilityHandler, otherwise use the BaseHandler from stdlib
try:
    from volatility3.framework.layers.resources import VolatilityHandler
except ImportError:
    # define dummy class
    # mypy checking: ignore name already defined
    class VolatilityHandler(BaseHandler):  # type: ignore
        pass


micro: Optional[Microvmi] = None


class MicrovmiHandlerError(Exception):
    pass


class VMIHandler(VolatilityHandler):
    """
    Handles the Virtual Machine Introspection URL scheme based on libmicrovmi

    Documentation: https://wenzel.github.io/libmicrovmi/

    Syntax is defined here:
        vmi://<hypervisor>/?param1=value1&param2=value2
    """

    SCHEME = "vmi"

    @classmethod
    def non_cached_schemes(cls) -> List[str]:
        return [VMIHandler.SCHEME]

    @staticmethod
    def vmi_open(req: Request) -> Optional[Any]:
        """Handles the request if it's the VMI scheme"""
        logging.getLogger("microvmi").setLevel(logging.WARNING)
        driver_type, init_params = url_to_driver_parameters(req.full_url)
        # this method is called multiple times
        # just return if already initialized instance
        global micro
        if micro is not None:
            return micro.padded_memory
        # init Microvmi
        micro = Microvmi(driver_type, init_params)
        return micro.padded_memory


def url_to_driver_parameters(url: str) -> Tuple[Optional[DriverType], Optional[DriverInitParamsPy]]:
    """Parses a given request and extracts the Microvmi driver initialization parameters"""
    parsed_url = urlparse(url)
    # scheme
    _validate_scheme(parsed_url.scheme)
    # hypervisor
    driver_type: Optional[DriverType] = _parse_hypervisor(parsed_url.netloc)
    # init params
    init_params: Optional[DriverInitParamsPy] = _parse_driver_init_params(parsed_url.query)
    return driver_type, init_params


def _validate_scheme(scheme: str):
    if scheme != VMIHandler.SCHEME:
        raise MicrovmiHandlerError(f"Scheme error: Got {scheme}, expected {VMIHandler.SCHEME}")


def _parse_vm_name(path: str) -> str:
    # remove /
    vm_name = path[1:]
    if not vm_name:
        raise MicrovmiHandlerError("Empty vm_name")
    return vm_name


def _parse_hypervisor(netloc: str) -> Optional[DriverType]:
    if netloc:
        try:
            drv_type = DriverType[netloc]
        except KeyError:
            raise MicrovmiHandlerError(
                f"Invalid driver type. Valid driver types: {' '.join([d.name for d in DriverType])}"
            )
        else:
            return drv_type
    return None


def _parse_driver_init_params(query: str) -> Optional[DriverInitParamsPy]:
    if not query:
        return None
    url_params: Dict[str, List[str]] = parse_qs(query, strict_parsing=True)
    if not url_params:
        return None
    common = None
    kvm = None
    memflow = None
    for param, list_value in url_params.items():
        if param == "vm_name":
            common = CommonInitParamsPy()
            common.vm_name = list_value[0]
        elif param == "kvm_unix_socket":
            kvm = KVMInitParamsPy()
            kvm.unix_socket = list_value[0]
        elif param == "memflow_connector_name":
            memflow = MemflowInitParamsPy(list_value[0])
        elif param == "memflow_connector_args":
            if memflow is None:
                raise MicrovmiHandlerError("memflow connector args received but no connector name specified")
            memflow.connector_args = list_value
        else:
            raise MicrovmiHandlerError(f"Unknown driver initialization parameter: {param}")
    init_params = DriverInitParamsPy()
    init_params.common = common
    init_params.kvm = kvm
    init_params.memflow = memflow
    return init_params
