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


def read_config(config_path: str):
    with open(config_path) as f:
        config = yaml.load(f, Loader=yaml.FullLoader)

    for nic in config['nics'].values():
        # Not support tas+cbs yet
        if 'tas' in nic and 'cbs' in nic:
            raise ValueError('Does not support tas + cbs yet')

        if 'tas' in nic:
            nic['tas']['txtime_delay'] = to_ns(nic['tas']['txtime_delay'])
            for sch in nic['tas']['schedule']:
                sch['time'] = to_ns(sch['time'])

        if 'cbs' in nic:
            for priomap in nic['cbs'].values():
                priomap['max_frame'] = to_kbps(priomap['max_frame'])
                priomap['bandwidth'] = to_kbps(priomap['bandwidth'])

    return config
