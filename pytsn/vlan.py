import shlex
import subprocess
import sys


def run_cmd(cmd: str):
    print(cmd, file=sys.stderr)
    return subprocess.check_call(shlex.split(cmd))


def vlan_name(ifname: str, vlanid) -> str:
    return f'{ifname}.{vlanid}'


def setup_mqprio(ifname: str, ifconf: dict):
    mqprio = ifconf['qdisc']['mqprio']
    root_handle = mqprio['handle']
    num_tc = mqprio['num_tc']
    priomap = ' '.join(map(str, mqprio['map']))
    queues = ' '.join(mqprio['queues'])
    run_cmd(
        f'tc qdisc add dev {ifname} parent root '
        f'handle {root_handle} mqprio '
        f'num_tc {num_tc} '
        f'map {priomap} '
        f'queues {queues} '
        f'hw 0'
    )

    for qid, val in ifconf['qdisc']['child'].items():
        t = val['type']
        handle = val['handle']
        if t == 'cbs':
            idleslope = val['idleslope']
            sendslope = val['sendslope']
            hicredit = val['hicredit']
            locredit = val['locredit']
            run_cmd(
                f'tc qdisc replace dev {ifname} '
                f'parent {root_handle}:{qid} '
                f'handle {handle} cbs '
                f'idleslope {idleslope} '
                f'sendslope {sendslope} '
                f'hicredit {hicredit} '
                f'locredit {locredit} '
                f'offload 1'
            )
            run_cmd(
                f'tc qdisc add dev {ifname} parent {handle}:1 etf '
                'clockid CLOCK_TAI '
                'delta 500000 '
                'offload '
            )
        else:
            raise ValueError(f'qdisc type {t} is not supported')


def setup_taprio(ifname: str, ifconf: dict):
    taprio = ifconf['qdisc']['taprio']
    handle = taprio['handle']
    num_tc = taprio['num_tc']
    priomap = ' '.join(map(str, taprio['map']))
    queues = ' '.join(taprio['queues'])
    base_time = taprio['base_time']
    sched_entries = ' '.join(f'sched-entry {entry}' for entry in taprio['sched_entries'])
    flags = taprio['flags']
    txtime_delay = taprio['txtime_delay']

    run_cmd(
        f'tc qdisc replace dev {ifname} parent root handle {handle} taprio '
        f'num_tc {num_tc} '
        f'map {priomap} '
        f'queues {queues} '
        f'base-time {base_time} '
        f'{sched_entries} '
        f'flags 0x{flags:x} '
        f'txtime-delay {txtime_delay} '
        f'clockid CLOCK_TAI'
    )
    run_cmd(
        f'tc qdisc replace dev {ifname} parent {handle}:1 etf '
        'clockid CLOCK_TAI '
        f'delta {txtime_delay} '
        'offload '
        'skip_sock_check'
    )


def create_vlan(config: dict, ifname: str, vlanid: int) -> int:
    name = vlan_name(ifname, vlanid)

    try:
        ifconf = config['nics'][ifname]

        qos_map = ' '.join(
            f'{skb_pri}:{vlan_pri}'
            for skb_pri, vlan_pri
            in ifconf['egress-qos-map'][vlanid].items())

        run_cmd(
            f'ip link add link {ifname} name {name} type vlan id {vlanid} '
            f'egress-qos-map {qos_map}'
        )

        run_cmd(
            f'ip link set up {name}'
        )

        if 'mqprio' in ifconf['qdisc']:
            setup_mqprio(ifname, ifconf)

        if 'taprio' in ifconf['qdisc']:
            setup_taprio(ifname, ifconf)
    except subprocess.CalledProcessError as e:
        return e.returncode
    except KeyError as e:
        print(e)
        print('Config is not properly configuered', file=sys.stderr)
        return 1
    else:
        return 0


def delete_vlan(config: dict, ifname: str, vlanid: int) -> int:
    name = vlan_name(ifname, vlanid)
    try:
        run_cmd(
            f'ip link del {name}'
        )

        run_cmd(
            f'tc qdisc delete dev {ifname} root'
        )
    except subprocess.CalledProcessError as e:
        return e.returncode
    else:
        return 0
