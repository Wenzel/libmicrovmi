#!/usr/bin/env python3

import argparse
import logging
from contextlib import contextmanager, nullcontext
from pathlib import Path
from typing import Optional

from microvmi import CommonInitParamsPy, DriverInitParamsPy, DriverType, KVMInitParamsPy, Microvmi, VBoxInitParamsPy
from rich import print
from rich.progress import BarColumn, Progress, TextColumn

READ_SIZE = 1024 * 1024


@contextmanager
def pause_ctxt(micro: Microvmi):
    micro.pause()
    try:
        yield micro
    finally:
        micro.resume()


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("-d", "--vm_name", type=str, help="The VM name to dump memory from")
    parser.add_argument("-k", "--kvm-unix-socket", type=str, help="The Unix socket path for the KVM driver")
    parser.add_argument(
        "--vbox-machinedebugger",
        action="store_true",
        help="Use the VirtualBox driver's MachineDebugger access method",
    )
    parser.add_argument(
        "--vbox-vmi",
        type=str,
        metavar="SOCKET",
        help="Use VBoxVMI (VM name defaults to the socket filename)",
    )
    parser.add_argument("--no-pause", action="store_true", help="Do not pause the VM while dumping")
    parser.add_argument("-o", "--output-file", help="Dump file path", default="{vm_name}.dump")
    args = parser.parse_args()
    if args.vbox_machinedebugger and args.vbox_vmi:
        parser.error("--vbox-machinedebugger and --vbox-vmi are mutually exclusive")
    if args.vm_name is None and args.vbox_vmi:
        args.vm_name = Path(args.vbox_vmi).stem
    return args


def dump_mem(
    vm_name: Optional[str],
    kvm_unix_socket: Optional[str],
    vbox_machinedebugger: bool,
    vbox_vmi: Optional[str],
    no_pause: bool,
    output_file: Optional[str],
):
    # prepare drivers init params
    init_params = DriverInitParamsPy()
    common = CommonInitParamsPy()
    common.vm_name = vm_name
    init_params.common = common
    if kvm_unix_socket:
        kvm = KVMInitParamsPy()
        kvm.unix_socket = kvm_unix_socket
        init_params.kvm = kvm
    driver_type = None
    if vbox_machinedebugger:
        init_params.virtualbox = VBoxInitParamsPy.MachineDebugger()
        # explicit driver type: surfaces the VirtualBox init error instead of NoDriverAvailable
        driver_type = DriverType.VirtualBox
    if vbox_vmi:
        init_params.virtualbox = VBoxInitParamsPy.VBoxVMI(socket_path=vbox_vmi)
        driver_type = DriverType.VirtualBox
    # init libmicrovmi
    micro = Microvmi(driver_type, init_params)
    destination = output_file.format(vm_name=vm_name)
    with nullcontext() if no_pause else pause_ctxt(micro):
        max_addr = micro.max_addr
        print(f"Dumping physical memory on {vm_name} until 0x{max_addr:X} to {destination}")
        with open(destination, "wb") as f:
            with Progress(
                "[progress.description]{task.description}",
                TextColumn("[bold yellow]0x{task.completed:X}"),
                BarColumn(bar_width=None),
                "[progress.percentage]{task.percentage:>3.0f}%",
                transient=True,
            ) as progress:
                dump_task = progress.add_task("Dumping ", total=max_addr)
                mem = micro.padded_memory
                for addr in range(0, max_addr, READ_SIZE):
                    logging.debug("dumping at 0x%x", addr)
                    current_chunk_size = min(READ_SIZE, max_addr - addr)
                    buffer = mem.read(current_chunk_size)
                    f.write(buffer)
                    progress.update(dump_task, advance=READ_SIZE)


def main():
    logging.basicConfig(level=logging.INFO)
    args = parse_args()

    try:
        vm_name = args.vm_name
        kvm_unix_socket = args.kvm_unix_socket
        output_file = args.output_file
        dump_mem(vm_name, kvm_unix_socket, args.vbox_machinedebugger, args.vbox_vmi, args.no_pause, output_file)
    except KeyboardInterrupt:
        logging.critical("Cancelled by CTRL-C")


if __name__ == "__main__":
    main()
