#!/usr/bin/env python3

import logging
import argparse
from contextlib import contextmanager

from microvmi import Microvmi
from rich.progress import Progress, BarColumn, TextColumn
from rich import print

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
    parser.add_argument("vm_name", help="The VM name to dump memory from")
    parser.add_argument(
        "-o", "--output-file", help="Dump file path", default="{vm_name}.dump"
    )
    args = parser.parse_args()
    return args


def dump_mem(vm_name, output_file):
    micro = Microvmi(vm_name)
    destination = output_file.format(vm_name=vm_name)
    with pause_ctxt(micro):
        max_addr = micro.max_addr
        print(
            f"Dumping physical memory on {vm_name} until 0x{max_addr:X} to {destination}"
        )
        with open(destination, "wb") as f:
            with Progress(
                "[progress.description]{task.description}",
                TextColumn("[bold yellow]0x{task.completed:X}"),
                BarColumn(bar_width=None),
                "[progress.percentage]{task.percentage:>3.0f}%",
                transient=True,
            ) as progress:
                dump_task = progress.add_task("Dumping ", total=max_addr)
                for addr in range(0, max_addr, READ_SIZE):
                    logging.debug("dumping at 0x%x", addr)
                    try:
                        buffer = micro.read_physical(addr, READ_SIZE)
                    except ValueError:
                        # write 0 page
                        buffer = bytes(READ_SIZE)
                    finally:
                        f.write(buffer)
                        progress.update(dump_task, advance=READ_SIZE)


def main():
    logging.basicConfig(level=logging.INFO)
    args = parse_args()

    try:
        vm_name = args.vm_name
        output_file = args.output_file
        dump_mem(vm_name, output_file)
    except KeyboardInterrupt:
        logging.critical("Cancelled by CTRL-C")


if __name__ == '__main__':
    main()
