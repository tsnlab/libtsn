import math

from typing import Dict, Iterable, Tuple

Credit = Dict[str, int]


def make_credit(sendslope, idleslope, hicredit, locredit):
    return {
        'sendslope': sendslope,
        'idleslope': idleslope,
        'hicredit': hicredit,
        'locredit': locredit,
    }


def calc_credits(streams: Dict[str, Iterable[dict]], linkspeed) -> Tuple[Credit, Credit]:
    idle_slope_a = sum(stream['bandwidth'] for stream in streams['a'])
    send_slope_a = idle_slope_a - linkspeed
    max_frame_a = sum(stream['max_frame'] for stream in streams['a'])
    hicredit_a = math.ceil(idle_slope_a * max_frame_a / linkspeed)
    locredit_a = math.ceil(send_slope_a * max_frame_a / linkspeed)
    credits_a = make_credit(send_slope_a // 1000, idle_slope_a // 1000, hicredit_a, locredit_a)

    idle_slope_b = sum(stream['bandwidth'] for stream in streams['b'])
    max_frame_b = sum(stream['max_frame'] for stream in streams['b'])
    send_slope_b = idle_slope_b - linkspeed
    hicredit_b = math.ceil(
        idle_slope_b *
        ((max_frame_b / (linkspeed - idle_slope_a)) +
         (max_frame_a / linkspeed)
         ))
    locredit_b = math.ceil(send_slope_b * max_frame_b / linkspeed)
    credits_b = make_credit(send_slope_b // 1000, idle_slope_b // 1000, hicredit_b, locredit_b)

    return credits_a, credits_b
