import argparse
import contextlib
import os
import re
import socket

if True:
    import sys

    # Trick for run on both python and zipapp
    sys.path.insert(0, os.path.dirname(__file__))

    from config import read_config
    from vlan import create_vlan, delete_vlan

SOCKET_PATH = '/var/run/tsn.sock'


def main():
    parser = argparse.ArgumentParser(description='Daemon for TSN traffic control')
    parser.add_argument('--config', '-c', default='config.yaml', help='Config file in yaml format')
    parser.add_argument('--bind', '-b', default=SOCKET_PATH, help='Unix socket path to bind')

    sub_parser = parser.add_subparsers(help='Directly create or delete vlan', dest='command')

    create_parser = sub_parser.add_parser('create')
    create_parser.add_argument('interface', help='Interface to create vlan')
    create_parser.add_argument('vlanid', type=int, help='Vlan id to create')

    delete_parser = sub_parser.add_parser('delete')
    delete_parser.add_argument('interface', help='Interface to delete vlan')
    delete_parser.add_argument('vlanid', type=int, help='Vlan id to delete')

    arguments = parser.parse_args()

    config = read_config(arguments.config)

    command_map = {
        'create': create_vlan,
        'delete': delete_vlan,
    }

    if arguments.command:
        return command_map[arguments.command](config, arguments.interface, arguments.vlanid)

    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(arguments.bind)
    server.listen(1)
    pattern = re.compile(r'(?P<cmd>create|delete) (?P<ifname>\w+) (?P<vlanid>\d+)')
    with contextlib.ExitStack() as es:
        def cleanup():
            server.close()
            os.remove(arguments.bind)

        es.callback(cleanup)

        while True:
            conn, addr = server.accept()
            line = conn.makefile().readline()
            print(f'{line=}')
            matched = pattern.match(line)
            if not matched:
                conn.send(b'-1')
            else:
                cmd = matched.group('cmd')
                ifname = matched.group('ifname')
                vlanid = int(matched.group('vlanid'))
                res = command_map[cmd](config, ifname, vlanid)
                conn.send(f'{res}'.encode())
            conn.close()


if __name__ == '__main__':
    exit(main())
