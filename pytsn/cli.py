import argparse
import contextlib
import os
import re
import socket

import watchgod
import yaml

from . config import read_config
from . vlan import create_vlan, delete_vlan

SOCKET_PATH = '/var/run/tsn.sock'


def info(ifname: str = None):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCKET_PATH)
    command = ('info' if not ifname else f'info {ifname}') + '\n'
    sock.send(command.encode())
    res = sock.makefile().read()
    sock.close()

    print(res)


def get_info(config: dict, ifname: str = None):
    def normalise(nic_conf: dict):
        result = {}
        if 'cbs' in nic_conf:
            cbs = nic_conf['cbs']
            classes = {}
            children = nic_conf['cbs']['children']
            if 1 in children:
                classes['a'] = {
                    'credits': children[1],
                    'prios': {prio: cbs[prio] for prio in range(16) if prio in cbs and cbs[prio]['class'] == 'a'},
                }
            if 2 in children:
                classes['b'] = {
                    'credits': children[2],
                    'prios': {prio: cbs[prio] for prio in range(16) if prio in cbs and cbs[prio]['class'] == 'b'},
                }
            result['cbs'] = classes
        if 'tas' in nic_conf:
            tas = nic_conf['tas']
            result['tas'] = {
                key: tas[key]
                for key in ('txtime_delay', 'base_time', 'schedule')
            }
        return result

    if not ifname:
        confs = config['nics']
    else:
        confs = {k: v for k, v in config['nics'].items() if k == ifname}

    return {ifname: normalise(conf) for ifname, conf in confs.items()}


command_map = {
    'create': create_vlan,
    'delete': delete_vlan,
}


def run_server(arguments):

    print('Loading server')

    config = read_config(arguments.config)
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(arguments.bind)
    server.listen(1)
    pattern_socket = re.compile(r'(?P<cmd>create|delete) (?P<ifname>\w+) (?P<vlanid>\d+)')
    pattern_info = re.compile(r'info(?: (?P<ifname>\w+))?')
    with contextlib.ExitStack() as es:
        def cleanup():
            server.close()
            os.remove(arguments.bind)

        es.callback(cleanup)

        try:
            while True:
                conn, addr = server.accept()
                line = conn.makefile().readline()
                print(f'line={line}')

                matched1 = pattern_socket.match(line)
                matched2 = pattern_info.match(line)

                if matched1:
                    cmd = matched1.group('cmd')
                    ifname = matched1.group('ifname')
                    vlanid = int(matched1.group('vlanid'))
                    res = command_map[cmd](config, ifname, vlanid)
                    conn.send(f'{res}'.encode())
                elif matched2:
                    ifname = matched2.group('ifname')
                    conn.send(yaml.safe_dump(get_info(config, ifname), default_flow_style=None).encode())
                else:
                    conn.send(b'-1')
                conn.close()
        except KeyboardInterrupt:
            # Not an actual KeyboardInterrupt. It is from watchgod
            pass


def main():
    parser = argparse.ArgumentParser(
        description='Daemon for TSN traffic control',
        formatter_class=argparse.ArgumentDefaultsHelpFormatter)
    parser.add_argument('--config', '-c', default='config.yaml', help='Config file in yaml format')
    parser.add_argument('--bind', '-b', default=SOCKET_PATH, help='Unix socket path to bind')

    sub_parser = parser.add_subparsers(help='Directly interact with CLI', dest='command')

    create_parser = sub_parser.add_parser('create', help='Create TSN interface from CLI')
    create_parser.add_argument('interface', help='Interface to create')
    create_parser.add_argument('vlanid', type=int, help='Vlan id to create')

    delete_parser = sub_parser.add_parser('delete', help='Delete TSN interface from CLI')
    delete_parser.add_argument('interface', help='Interface to delete')
    delete_parser.add_argument('vlanid', type=int, help='Vlan id to delete')

    info_parser = sub_parser.add_parser('info', help='Get information about TSN interface')
    info_parser.add_argument('interface', nargs='?', help='Interface to get info')

    info_parser = sub_parser.add_parser('daemon', help='Start TSN UCD daemon')

    arguments = parser.parse_args()

    if arguments.command in ('create', 'delete'):
        config = read_config(arguments.config)
        return command_map[arguments.command](config, arguments.interface, arguments.vlanid)
    elif arguments.command == 'info':
        return info(arguments.interface)

    watchgod.run_process(arguments.config, run_server, args=(arguments,))


if __name__ == '__main__':
    exit(main())
