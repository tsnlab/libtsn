#!/usr/bin/env -S python3 -u

import argparse
import shlex
import subprocess
import sys
import functools
import logging
import os

from contextlib import ExitStack

logger = logging.getLogger(__name__)


def run_cmd(cmd: str) -> str:
    return subprocess.check_output(shlex.split(cmd)).decode('utf-8')


def vlan_ifname(ifname: str, vlanid: int) -> str:
    return f'{ifname}.{vlanid}'


def make_vlan(ifname: str, vlanid: int):
    name = vlan_ifname(ifname, vlanid)
    logger.info(f'Creating vlan interface {name}')

    # TODO: set priority map
    qos_map = ' '.join(f'{i}:{i}' for i in range(8))
    run_cmd(f'ip link add link {ifname} name {name} type vlan id {vlanid} egress-qos-map')
    run_cmd(f'ip link set up {name}')


def del_vlan(ifname: str, vlanid: int):
    name = vlan_ifname(ifname, vlanid)
    logger.info(f'Deleting vlan interface {name}')
    run_cmd(f'ip link delete {name}')


def main():
    loglevel = os.getenv('TSN_LOGLEVEL', 'INFO')
    logging.basicConfig(
        format='%(asctime)s:%(name)s:%(levelname)s: %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S %z',
        level=loglevel)
    logger.setLevel(loglevel)

    parser = argparse.ArgumentParser()
    parser.add_argument('-v', '--vlan', action='append', nargs=2, metavar=('ifname', 'vlanid'))
    parser.add_argument('command', nargs='*')

    parsed = parser.parse_args()

    with ExitStack() as estack:
        for vlan in parsed.vlan:
            make_vlan(*vlan)
            estack.callback(functools.partial(del_vlan, *vlan))

        p = subprocess.Popen(parsed.command)
        p.wait()


if __name__ == '__main__':
    sys.exit(main())
