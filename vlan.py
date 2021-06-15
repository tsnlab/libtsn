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

        ifconf = config['nics'][ifname]

        qos_map = ' '.join(
            f'{skb_pri}:{vlan_pri}'
            for skb_pri, vlan_pri
            in ifconf['egress-qos-map'][vlanid].items())

        subprocess.check_call(shlex.split(
            f'ip link add link {ifname} name {name} type vlan id {vlanid} '
            f'egress-qos-map {qos_map}'
        ))

        subprocess.check_call(shlex.split(
            f'ip link set up {name}'
        ))

        # Set mqprio
        mqprio = ifconf['qdisc']['mqprio']
        root_handle = mqprio['handle']
        num_tc = mqprio['num_tc']
        priomap = ' '.join(map(str, mqprio['map']))
        queues = ' '.join(mqprio['queues'])
        offload = mqprio.get('hw', False)
        subprocess.check_call(shlex.split(
            f'tc qdisc add dev {ifname} parent root '
            f'handle {root_handle} mqprio '
            f'num_tc {num_tc} '
            f'map {priomap} '
            f'queues {queues} '
            f'hw {1 if offload else 0}'
        ))

        for qid, val in ifconf['qdisc']['child'].items():
            t = val['type']
            handle = val['handle']
            if t == 'cbs':
                idleslope = val['idleslope']
                sendslope = val['sendslope']
                hicredit = val['hicredit']
                locredit = val['locredit']
                subprocess.check_call(shlex.split(
                    f'tc qdisc replace dev {ifname} '
                    f'parent {root_handle}:{qid} '
                    f'handle {handle} cbs '
                    f'idleslope {idleslope} '
                    f'sendslope {sendslope} '
                    f'hicredit {hicredit} '
                    f'locredit {locredit} '
                    f'offload 1'
                ))
                subprocess.check_call(shlex.split(
                    f'tc qdisc add dev {ifname} parent {handle}:1 etf '
                    'clockid CLOCK_TAI '
                    'delta 500000 '
                    'offload '
                ))
            else:  # TODO: support taprio
                raise ValueError(f'qdisc type {t} is not supported')
    except subprocess.CalledProcessError as e:
        return e.returncode
    except KeyError as e:
        print(e)
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

        subprocess.check_call(shlex.split(
            f'tc qdisc delete dev {ifname} root'
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
