#!/usr/bin/env -S python3 -u

import argparse
import logging
import shlex
import subprocess
import sys

from contextlib import ExitStack
from typing import Dict

import yaml

logger = logging.getLogger("TSN")


def run_cmd(cmd: str) -> str:
    return subprocess.check_output(shlex.split(cmd)).decode('utf-8')


def vlan_ifname(ifname: str, vlanid: int) -> str:
    return f'{ifname}.{vlanid}'


def make_vlan(ifname: str, vlanid: int, qosmap: Dict[int, int]):
    name = vlan_ifname(ifname, vlanid)

    qosmap_str = ' '.join(
        f'{k}:{v}'
        for k, v in qosmap.items()
    )

    logger.info(f'Creating vlan interface {name} with qosmap {qosmap_str}')

    run_cmd(
        f'ip link add link {ifname} name {name} type vlan id {vlanid} '
        f'egress-qos-map {qosmap_str}')
    run_cmd(f'ip link set up {name}')


def del_vlan(ifname: str, vlanid: int):
    name = vlan_ifname(ifname, vlanid)
    logger.info(f'Deleting vlan interface {name}')
    run_cmd(f'ip link delete {name}')


def setup(config):
    logger.info("Setting up environments")

    for nic in config.get('nics', []):
        make_vlan(nic['ifname'], nic['vlanid'], nic['qosmap'])


def cleanup(config):
    logger.info("Cleaning up environments")

    for nic in config.get('nics', []):
        del_vlan(nic['ifname'], nic['vlanid'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        '-c', '--config',
        default='config.yaml',
        help='YAML config filename')
    parser.add_argument(
        'command', nargs='+',
        help='Command to execute. prepend -- before command to prevent problems')

    parsed = parser.parse_args()

    with open(parsed.config) as f:
        config = yaml.load(f, Loader=yaml.FullLoader)

    handler = logging.StreamHandler()
    handler.formatter = logging.Formatter(
        '%(asctime)s:%(name)s:%(levelname)s: %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S %z')
    logger.addHandler(handler)
    logger.setLevel(config.get('log_level', 'INFO'))

    with ExitStack() as estack:
        estack.callback(cleanup, config)
        setup(config)

        logger.info("Starting application")
        p = subprocess.Popen(parsed.command)
        p.wait()


if __name__ == '__main__':
    sys.exit(main())
