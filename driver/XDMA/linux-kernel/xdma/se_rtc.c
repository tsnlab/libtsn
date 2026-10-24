// SPDX-License-Identifier: GPL-2.0
/*
 * se_rtc.c - HAT 모드 수신 하드웨어 시각을 gPTP 시각으로 바꾼다(방법 1, 2026-10-12).
 * 설명은 se_rtc.h 를 본다.
 *
 * 받는 규칙(10-12 독립 리뷰 반영):
 * - 1 ms 에 한 번만 읽는다(offset 은 1 ms 에 1 ns 안팎 움직임, PCIe 읽기 비용 절감).
 * - 두 번 연속 같은 값이 읽혀야 한다(래치·펌웨어 쓰기와 겹친 읽기 제외).
 * - 마지막으로 받은 값과 비교한 허용 폭 = 1 ms + 경과 시간 x 20 ppm(발진기 차 여유).
 *   오래 쉬었다가 읽어도 정상 drift 를 큰 변화로 거부하지 않는다.
 * - 허용 폭을 넘는 값, 첫 값, 60 s 넘게 쉰 뒤의 값은 3 ms 뒤 같은 값이 다시 읽힐 때 받는다.
 *   확인 중에는 se_rtc_usable() 이 false 라 호출자가 시각을 붙이지 않는다.
 *
 * 한계(10-12 재리뷰): offset 이 크게 바뀌는(서보 위상 점프, GM 교체) 순간 앞뒤 약 1~3 ms 에
 * 처리된 프레임은 바뀌기 전 또는 뒤 offset 이 붙어 그 점프 크기만큼 틀릴 수 있다.
 * 레지스터를 읽는 시점과 프레임 수신 시점이 다르기 때문이다. 평상시 drift(1.4 ms 에 -1 ns)의 오차는 2 ns 안팎이다.
 */
#include <linux/io.h>
#include <linux/timekeeping.h>
#include <linux/math64.h>
#include "se_rtc.h"

#define NSEC_PER_SEC_U32	1000000000U
#define SE_RTC_JUMP_NS		1000000LL	/* 1 ms */
#define SE_RTC_DRIFT_PPM	20		/* 경과 시간 1 s 당 20 us 더 허용 */
#define SE_RTC_CONFIRM_NS	3000000ULL	/* 3 ms */
#define SE_RTC_MIN_PERIOD_NS	1000000ULL	/* 1 ms */
#define SE_RTC_READ_TRIES	3
#define SE_RTC_MAX_IDLE_MS	60000ULL	/* 60 s 넘게 쉬었으면 확인 경로로 받는다 */

static void read_once(void __iomem *rtc, u32 *sh, u32 *sl, u32 *ns)
{
	*sh = ioread32(rtc + SE_RTC_OFS_SH) & 0xFFFF;
	*sl = ioread32(rtc + SE_RTC_OFS_SL);
	*ns = ioread32(rtc + SE_RTC_OFS_NS) & 0x3FFFFFFF;
}

s64 se_rtc_ofs_diff_ns(const struct se_rtc_ofs *a, const struct se_rtc_ofs *b)
{
	s64 ds = a->sec - b->sec;

	if (ds > 2 || ds < -2)
		return ds > 0 ? S64_MAX : S64_MIN;
	return ds * NSEC_PER_SEC + (s64)a->ns - (s64)b->ns;
}

static bool within(const struct se_rtc_ofs *a, const struct se_rtc_ofs *b, s64 lim)
{
	s64 d = se_rtc_ofs_diff_ns(a, b);

	return d <= lim && d >= -lim;
}

struct se_rtc_ofs se_rtc_ofs_from_regs(u32 sh, u32 sl, u32 ns)
{
	struct se_rtc_ofs v;

	v.sec = (s64)(((u64)(sh & 0xFFFF) << 32) | sl);
	if (sh & 0x8000)
		v.sec -= (s64)1 << 48;	/* 48 비트 부호 확장 */
	v.ns = ns;
	return v;
}

bool se_rtc_ofs_update(void __iomem *bar0, struct se_rtc_state *st)
{
	void __iomem *rtc = bar0 + SE_RTC_BASE;
	u32 sh, sl, ns, sh2, sl2, ns2;
	struct se_rtc_ofs v;
	u64 now = ktime_get_ns();
	u64 idle_ms;
	s64 lim;
	int i;

	if (st->last_read_ns && now - st->last_read_ns < SE_RTC_MIN_PERIOD_NS)
		return se_rtc_usable(st);
	st->last_read_ns = now;

	read_once(rtc, &sh, &sl, &ns);
	for (i = 0; i < SE_RTC_READ_TRIES; i++) {
		read_once(rtc, &sh2, &sl2, &ns2);
		if (sh == sh2 && sl == sl2 && ns == ns2)
			break;
		st->retries++;
		sh = sh2;
		sl = sl2;
		ns = ns2;
	}
	if (i == SE_RTC_READ_TRIES || ns >= NSEC_PER_SEC_U32) {
		st->rejects++;
		return se_rtc_usable(st);
	}
	v = se_rtc_ofs_from_regs(sh, sl, ns);

	idle_ms = div_u64(now - st->last_ok_ns, 1000000);
	if (st->valid && idle_ms <= SE_RTC_MAX_IDLE_MS) {
		/* 1 ms 동안 1 ppm = 1 ns. 60 s 면 2.2 ms */
		lim = SE_RTC_JUMP_NS + (s64)idle_ms * SE_RTC_DRIFT_PPM;
		if (within(&v, &st->cur, lim)) {
			st->cur = v;
			st->last_ok_ns = now;
			st->has_cand = false;
			st->updates++;
			return true;
		}
	}

	/* 첫 값 또는 큰 변화: 시간을 두고 같은 값이 다시 보일 때만 받는다 */
	if (st->has_cand && within(&v, &st->cand, SE_RTC_JUMP_NS) &&
	    now - st->cand_ns >= SE_RTC_CONFIRM_NS) {
		st->cur = v;
		st->valid = true;
		st->last_ok_ns = now;
		st->has_cand = false;
		st->updates++;
		return true;
	}
	if (!st->has_cand || !within(&v, &st->cand, SE_RTC_JUMP_NS)) {
		st->cand = v;
		st->cand_ns = now;
		st->has_cand = true;
	}
	st->rejects++;
	return false;
}

ktime_t se_rtc_to_gptp(u64 raw_sec, u32 raw_ns, const struct se_rtc_ofs *ofs)
{
	s64 sec = (s64)raw_sec + ofs->sec;
	u64 ns = (u64)(raw_ns & 0x3FFFFFFF) + ofs->ns;

	/* raw_ns 와 ofs->ns 는 각각 1e9 미만이라 자리올림은 한 번이면 된다 */
	if (ns >= NSEC_PER_SEC_U32) {
		ns -= NSEC_PER_SEC_U32;
		sec++;
	}
	return ktime_set(sec, (unsigned long)ns);
}
