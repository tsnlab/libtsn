import math

from dataclasses import asdict, dataclass
from typing import Dict, Iterable, Tuple

frame_non_sr = 1542


@dataclass
class Credits():
    sendslope: int
    idleslope: int
    hicredit: int
    locredit: int


def calc_credits(streams: Dict[str, Iterable[dict]], linkspeed) -> Tuple[Credits, Credits]:
    idle_slope_a = sum(stream['bandwidth'] for stream in streams['a'])
    send_slope_a = idle_slope_a - linkspeed
    max_frame_a = sum(stream['max_frame'] for stream in streams['a'])
    hicredit_a = math.ceil(idle_slope_a * frame_non_sr / linkspeed)
    locredit_a = math.ceil(send_slope_a * max_frame_a / linkspeed)
    credits_a = Credits(send_slope_a, idle_slope_a, hicredit_a, locredit_a)

    idle_slope_b = sum(stream['bandwidth'] for stream in streams['b'])
    max_frame_b = sum(stream['max_frame'] for stream in streams['b'])
    send_slope_b = idle_slope_b - linkspeed
    hicredit_b = math.ceil(
        idle_slope_b *
        ((frame_non_sr / (linkspeed - idle_slope_a)) +
         (max_frame_a / linkspeed)
         ))
    locredit_b = math.ceil(send_slope_b * max_frame_b / linkspeed)
    credits_b = Credits(send_slope_b, idle_slope_b, hicredit_b, locredit_b)

    return asdict(credits_a), asdict(credits_b)
