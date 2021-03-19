import logging
from typing import Any, Optional, Tuple
from urllib.parse import parse_qs, urlparse
from urllib.request import BaseHandler, Request

from microvmi import DriverInitParam, DriverType, Microvmi

micro: Optional[Microvmi] = None


class MicrovmiHandlerError(Exception):
    pass


class VMIHandler(BaseHandler):
    """
    Handles the Virtual Machine Introspection URL scheme based on libmicrovmi

    Documentation: https://wenzel.github.io/libmicrovmi/

    Syntax is defined here:
        vmi://<hypervisor>/<vm_name>?param1=value1&param2=value2
    """

    SCHEME = "vmi"

    """Map of driver initialization parameter keys to the associated DriverInitParam functions
    exposed by the pymicrovmi python extension"""
    DRIVER_INIT_PARAM_MAP = {"kvmi_unix_socket": DriverInitParam.kvmi_unix_socket}

    @staticmethod
    def vmi_open(req: Request) -> Optional[Any]:
        """Handles the request if it's the VMI scheme"""
        logging.getLogger("microvmi").setLevel(logging.WARNING)
        vm_name, driver_type, init_param = url_to_driver_parameters(req.full_url)
        # this method is called multiple times
        # just return if already initialized instance
        global micro
        if micro is not None:
            return micro.padded_memory
        # init Microvmi
        micro = Microvmi(vm_name, driver_type, init_param)
        return micro.padded_memory


def url_to_driver_parameters(url: str) -> Tuple[str, Optional[DriverType], Optional[DriverInitParam]]:
    """Parses a given request and extracts the Microvmi driver initialization parameters"""
    parsed_url = urlparse(url)
    # scheme
    _validate_scheme(parsed_url.scheme)
    # domain
    vm_name: str = _parse_vm_name(parsed_url.path)
    # hypervisor
    driver_type: Optional[DriverType] = _parse_hypervisor(parsed_url.netloc)
    # init params
    init_param: Optional[DriverInitParam] = _parse_driver_init_params(parsed_url.query)
    return vm_name, driver_type, init_param


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


def _parse_driver_init_params(query: str) -> Optional[DriverInitParam]:
    if not query:
        return None
    url_params = parse_qs(query, strict_parsing=True)
    if not url_params:
        return None
    if len(url_params) > 1:
        raise MicrovmiHandlerError("Only one driver initialization parameter is supported")
    try:
        key = list(url_params.keys())[0]
        init_param_func = VMIHandler.DRIVER_INIT_PARAM_MAP[key]
    except KeyError as e:
        raise MicrovmiHandlerError(
            f"Unknown driver initialization parameter. Allow init parameters: {VMIHandler.DRIVER_INIT_PARAM_MAP.keys()}"
        ) from e
    else:
        return init_param_func(url_params[key][0])
