#!/usr/bin/env python3

import argparse
import logging
from contextlib import contextmanager
from typing import Optional

from microvmi import CommonInitParamsPy, DriverInitParamsPy, KVMInitParamsPy, Microvmi
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
    parser.add_argument("-o", "--output-file", help="Dump file path", default="{vm_name}.dump")
    args = parser.parse_args()
    return args


def dump_mem(vm_name: Optional[str], kvm_unix_socket: Optional[str], output_file: Optional[str]):
    # prepare drivers init params
    init_params = DriverInitParamsPy()
    common = CommonInitParamsPy()
    common.vm_name = vm_name
    init_params.common = common
    if kvm_unix_socket:
        kvm = KVMInitParamsPy()
        kvm.unix_socket = kvm_unix_socket
        init_params.kvm = kvm
    # init libmicrovmi
    micro = Microvmi(None, init_params)
    destination = output_file.format(vm_name=vm_name)
    with pause_ctxt(micro):
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
        dump_mem(vm_name, kvm_unix_socket, output_file)
    except KeyboardInterrupt:
        logging.critical("Cancelled by CTRL-C")


if __name__ == "__main__":
    main()
