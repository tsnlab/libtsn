#!/usr/bin/env -S python3 -u
import argparse
import shlex
import subprocess
import sys

import yaml


def vlan_name(ifname: str, vlanid) -> str:
    return f'{ifname}.{vlanid}'


def create_vlan(ifname: str, vlanid: int) -> int:
    name = vlan_name(ifname, vlanid)

    try:
        with open('config.yaml') as f:
            config = yaml.load(f, Loader=yaml.FullLoader)

        qos_map = ' '.join(
            f'{skb_pri}:{vlan_pri}'
            for skb_pri, vlan_pri
            in config['nics'][ifname]['egress-qos-map'][vlanid].items())

        subprocess.check_call(shlex.split(
            # TODO: egress-qos-map
            f'ip link add link {ifname} name {name} type vlan id {vlanid} '
            f'egress-qos-map {qos_map}'
        ))
        subprocess.check_call(shlex.split(
            f'ip link set up {name}'
        ))
    except subprocess.CalledProcessError as e:
        return e.returncode
    except KeyError as e:
        print('Config is not properly configuered', file=sys.stderr)
        return 1
    else:
        return 0


def delete_vlan(ifname: str, vlanid: int) -> int:
    name = vlan_name(ifname, vlanid)
    try:
        subprocess.check_call(shlex.split(
            f'ip link del {name}'
        ))
    except subprocess.CalledProcessError as e:
        return e.returncode
    else:
        return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=('create', 'delete'))
    parser.add_argument('ifname')
    parser.add_argument('vlanid', type=int)

    args = parser.parse_args()

    {
        'create': create_vlan,
        'delete': delete_vlan,
    }[args.command](args.ifname, args.vlanid)


if __name__ == '__main__':
    exit(main())
