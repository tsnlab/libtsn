import argparse
import contextlib
import os
import re
import socket

import yaml

if True:
    import sys

    # Trick for run on both python and zipapp
    sys.path.insert(0, os.path.dirname(__file__))

    from config import read_config
    from vlan import create_vlan, delete_vlan

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
    if not ifname:
        return config['nics']
    else:
        return {k: v for k, v in config['nics'].items() if k == ifname}


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

    command_map = {
        'create': create_vlan,
        'delete': delete_vlan,
    }

    if arguments.command in ('create', 'delete'):
        config = read_config(arguments.config)
        return command_map[arguments.command](config, arguments.ifname, arguments.vlanid)
    elif arguments.command == 'info':
        return info(arguments.interface)

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

        while True:
            conn, addr = server.accept()
            line = conn.makefile().readline()
            print(f'{line=}')

            if matched := pattern_socket.match(line):
                cmd = matched.group('cmd')
                ifname = matched.group('ifname')
                vlanid = int(matched.group('vlanid'))
                res = command_map[cmd](config, ifname, vlanid)
                conn.send(f'{res}'.encode())
            elif matched := pattern_info.match(line):
                ifname = matched.group('ifname')
                conn.send(yaml.dump(get_info(config, ifname)).encode())
            else:
                conn.send(b'-1')
            conn.close()


if __name__ == '__main__':
    exit(main())
