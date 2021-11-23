import re
import subprocess

from typing import Union

import yaml

from . cbs import calc_credits

modifier_map = {
    '': 1,
    'k': 1_000 ** 1,
    'M': 1_000 ** 2,
    'G': 1_000 ** 3,
    'ki': 1024 ** 1,
    'Mi': 1024 ** 2,
    'Gi': 1024 ** 3,
}

bits_map = {
    'b': 1,
    'B': 8,
}


def to_ns(value: Union[int, str]) -> int:
    if isinstance(value, int):
        return value
    elif isinstance(value, str):
        matched = re.match(r'^(?P<v>[\d_]+)\s*(?P<unit>|ns|us|µs|ms)$', value)
        if not matched:
            raise ValueError(f"{value} is not valid time")
        v = int(matched.group('v').replace('_', ''))
        unit = matched.group('unit')
        return {
            '': 1,
            'ns': 1,
            'us': 1_000,
            'µs': 1_000,
            'ms': 1_000_000,
        }[unit] * v


def to_bps(value: Union[int, str]) -> int:
    if isinstance(value, int):
        return value
    elif isinstance(value, str):
        matched = re.match(r'^(?P<v>[\d_]+)\s*(?P<modifier>|k|M|G)(?P<b>b|B)[p\/]s$', value)
        if not matched:
            raise ValueError(f"{value} is not valid bandwidth")
        v = int(matched.group('v').replace('_', ''))
        bits_or_bytes = matched.group('b')
        unit = matched.group('modifier')

        return v * modifier_map[unit] * bits_map[bits_or_bytes]


def to_bits(value: Union[int, str]) -> int:
    if isinstance(value, int):
        return value
    elif isinstance(value, str):
        matched = re.match(r'^(?P<v>[\d_]+)\s*(?P<modifier>|k|M|G|ki|Mi|Gi)(?P<b>b|B)$', value)
        if not matched:
            raise ValueError(f"{value} is not valid size")
        v = int(matched.group('v').replace('_', ''))
        bits_or_bytes = matched.group('b')
        unit = matched.group('modifier')

        return v * modifier_map[unit] * bits_map[bits_or_bytes]


def get_linkspeed(ifname: str) -> str:
    output = subprocess.check_output(['ethtool', ifname], stderr=subprocess.DEVNULL).decode()
    pattern = re.compile(r'Speed: (?P<speed>\d+(?:|k|M|G)b[p\/]s)')
    matched = pattern.search(output)

    if not matched:
        raise OSError(f'Failed to get linkspeed of {ifname}')

    return matched.group('speed')


def normalise_tas(ifname: str, config: dict) -> dict:
    # TODO: Make offload flag

    config['txtime_delay'] = to_ns(config['txtime_delay'])
    tc_map = {}

    for sch in config['schedule']:
        sch['time'] = to_ns(sch['time'])
        for prio in sch['prio']:
            if prio > 0 and prio not in tc_map:
                tc_map[prio] = len(tc_map)

    tc_map[-1] = len(tc_map)  # BE
    num_tc = len(tc_map)

    config['tc_map'] = [tc_map.get(prio, tc_map[-1]) for prio in range(16)]
    config['num_tc'] = num_tc
    config['queues'] = ['1@0'] * num_tc
    config['base_time'] = 0
    config['sched_entries'] = [
        f"S {sum(1 << tc_map[pri] for pri in sch['prio'])} {sch['time']}"
        for sch in config['schedule']
    ]

    return config


def normalise_cbs(ifname: str, config: dict) -> dict:
    # TODO: Get queue count from ifname

    tc_map = {}
    children = {}
    try:
        linkspeed = to_bps(get_linkspeed(ifname))
    except (OSError, subprocess.CalledProcessError):
        linkspeed = to_bps('1000Mbps')
    streams = {
        'a': [],
        'b': [],
    }

    for prio, priomap in config.items():
        if prio not in tc_map:
            tc_map[prio] = len(tc_map)

        child = {
            'max_frame': to_bits(priomap['max_frame']),
            'bandwidth': to_bps(priomap['bandwidth']),
        }

        # TODO: validate class
        streams[priomap['class']].append(child)

    children[1], children[2] = calc_credits(streams, linkspeed)

    tc_map[-1] = len(tc_map)  # BE
    num_tc = len(tc_map)

    config['tc_map'] = [tc_map.get(prio, tc_map[-1]) for prio in range(16)]
    config['num_tc'] = num_tc
    config['queues'] = [f'1@{i}' for i in range(num_tc)]  # FIXME: Fill out remaining queues
    config['children'] = children

    return config


def read_config(config_path: str):
    with open(config_path) as f:
        config = yaml.safe_load(f)

    for ifname, ifconfig in config['nics'].items():
        if 'tas' in ifconfig:
            ifconfig['tas'] = normalise_tas(ifname, ifconfig['tas'])

        if 'cbs' in ifconfig:
            ifconfig['cbs'] = normalise_cbs(ifname, ifconfig['cbs'])

    return config
