import re

from typing import Union

import yaml


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


def to_kbps(value: Union[int, str]) -> int:
    if isinstance(value, int):
        return value
    elif isinstance(value, str):
        matched = re.match(r'^(?P<v>[\d_]+)\s*(?P<unit>bps|kbps|Mbps|Gbps)$', value)
        if not matched:
            raise ValueError(f"{value} is not valid bandwidth")
        v = int(matched.group('v').replace('_', ''))
        unit = matched.group('unit')
        return int({
            'bps': 0.001,
            'kbps': 1,
            'Mbps': 1_000,
            'Gbps': 1_000,
        }[unit] * v)


def normalise_tas(config: dict) -> dict:
    #TODO: Make offload flag

    config['txtime_delay'] = to_ns(config['txtime_delay'])
    tc_map = {}

    for sch in config['schedule']:
        sch['time'] = to_ns(sch['time'])
        for prio in sch['prio']:
            if prio > 0 and prio not in tc_map:
                tc_map[prio] = len(tc_map)

    tc_map[-1] = len(tc_map)  # BE
    num_tc = len(tc_map)

    config['handle'] = 100  # TODO: make unique
    config['tc_map'] = [tc_map.get(prio, tc_map[-1]) for prio in range(16)]
    config['num_tc'] = num_tc
    config['queues'] = ['1@0'] * num_tc
    config['base_time'] = 0
    config['sched_entries'] = [
        f"S {sum(1 << tc_map[pri] for pri in sch['prio'])} {sch['time']}"
        for sch in config['schedule']
    ]

    return config


def normalise_cbs(config: dict) -> dict:
    for priomap in config.values():
        priomap['max_frame'] = to_kbps(priomap['max_frame'])
        priomap['bandwidth'] = to_kbps(priomap['bandwidth'])

    return config


def read_config(config_path: str):
    with open(config_path) as f:
        config = yaml.load(f, Loader=yaml.FullLoader)

    for nic in config['nics'].values():
        # Not support tas+cbs yet
        if 'tas' in nic and 'cbs' in nic:
            raise ValueError('Does not support tas + cbs yet')

        if 'tas' in nic:
            nic['tas'] = normalise_tas(nic['tas'])

        if 'cbs' in nic:
            nic['cbs'] = normalise_cbs(nic['cbs'])

    return config
