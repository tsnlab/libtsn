import shlex
import subprocess
import sys


def run_cmd(cmd: str):
    print(cmd, file=sys.stderr)
    return subprocess.check_call(shlex.split(cmd))


def vlan_name(ifname: str, vlanid) -> str:
    return f'{ifname}.{vlanid}'


def setup_tas(ifname: str, tas: dict):
    handle = 100
    num_tc = tas['num_tc']
    priomap = ' '.join(map(str, tas['tc_map']))
    queues = ' '.join(tas['queues'])
    base_time = tas['base_time']
    sched_entries = ' '.join(f'sched-entry {entry}' for entry in tas['sched_entries'])
    txtime_delay = tas['txtime_delay']

    run_cmd(
        f'tc qdisc replace dev {ifname} parent root handle {handle} taprio '
        f'num_tc {num_tc} '
        f'map {priomap} '
        f'queues {queues} '
        f'base-time {base_time} '
        f'{sched_entries} '
        f'flags 0x0 '
        f'txtime-delay {txtime_delay} '
        f'clockid CLOCK_TAI'
    )

    run_cmd(
        f'tc qdisc replace dev {ifname} parent {handle}:1 etf '
        'clockid CLOCK_TAI '
        f'delta {txtime_delay} '
        'skip_sock_check'
    )


def setup_cbs(ifname: str, cbs: dict):
    root_handle = 100
    num_tc = cbs['num_tc']
    priomap = ' '.join(map(str, cbs['tc_map']))
    queues = ' '.join(cbs['queues'])
    run_cmd(
        f'tc qdisc add dev {ifname} parent root '
        f'handle {root_handle} mqprio '
        f'num_tc {num_tc} '
        f'map {priomap} '
        f'queues {queues} '
        f'hw 0'
    )

    for qid, val in cbs['children'].items():
        handle = qid * 1111

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
            f'offload 0'
        )
        # run_cmd(
        #     f'tc qdisc add dev {ifname} parent {handle}:1 etf '
        #     'clockid CLOCK_TAI '
        #     'delta 500000 '
        #     'offload '
        # )


def create_vlan(config: dict, ifname: str, vlanid: int) -> int:
    name = vlan_name(ifname, vlanid)

    try:
        ifconf = config['nics'][ifname]

        # Not support tas+cbs yet
        if all(x in ifconf for x in ('tas', 'cbs')):
            raise ValueError('Does not support tas + cbs yet')

        qos_map = ' '.join(
            f'{skb_pri}:{vlan_pri}'
            for skb_pri, vlan_pri
            in ifconf['vlan'][vlanid]['maps'].items())

        ipv4 = ifconf['vlan'][vlanid].get('ipv4', None)

        run_cmd(
            f'ip link add link {ifname} name {name} type vlan id {vlanid} '
            f'egress-qos-map {qos_map}'
        )

        run_cmd(
            f'ip link set up {name}'
        )

        if ipv4:
            run_cmd(
                f'ip addr add {ipv4} dev {name}'
            )

        if 'tas' in ifconf:
            setup_tas(ifname, ifconf['tas'])

        if 'cbs' in ifconf:
            setup_cbs(ifname, ifconf['cbs'])
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
