/* SPDX-License-Identifier: GPL-2.0 */
/*
 * se_rtc.h - HAT 모드 수신 하드웨어 시각을 gPTP 시각으로 바꾼다(방법 1, 2026-10-12).
 *
 * TSN 코어의 수신 시각은 자유 구동 로컬 카운터(curtime)다. gPTP 시각은
 * curtime + RTC offset 이다. 펌웨어(gPTP 서보)가 offset 을 고친다.
 * 드라이버는 RTC 블록의 offset 레지스터를 읽기만 한다.
 *
 * - 0x10(CT_NS)에 쓰지 않는다. 그 쓰기는 offset 레지스터를 덮어써서
 *   펌웨어의 위상 쓰기(ns -> sec 하위 -> sec 상위)와 섞일 수 있다.
 * - 펌웨어는 약 1.4 ms 마다 0x10 래치로 offset 레지스터를 새 값으로 바꾼다
 *   (2026-10-12 읽기 전용 표본). 그래서 읽은 값의 지연은 2 ms 이하다.
 * - 부동소수점·SIMD 를 쓰지 않는다(정수 연산만).
 */
#ifndef SE_RTC_H
#define SE_RTC_H

#include <linux/types.h>
#include <linux/ktime.h>

/* RTC 블록(두 포트가 같은 RTC 를 씀, ETH1 인스턴스) BAR0 위치 */
#define SE_RTC_BASE		0x20800
#define SE_RTC_OFS_NS		0x20	/* [29:0] ns */
#define SE_RTC_OFS_SL		0x24	/* 초 하위 32 비트 */
#define SE_RTC_OFS_SH		0x28	/* [15:0] 초 상위 16 비트 */

struct se_rtc_ofs {
	s64 sec;		/* 48 비트 부호 확장 */
	u32 ns;			/* 0 ~ 999,999,999 */
};

struct se_rtc_state {
	struct se_rtc_ofs cur;		/* 현재 쓰는 offset */
	struct se_rtc_ofs cand;		/* 큰 변화 후보 */
	u64 cand_ns;			/* 후보를 처음 본 시각(ktime_get_ns) */
	u64 last_read_ns;		/* 마지막으로 레지스터를 읽은 시각 */
	u64 last_ok_ns;			/* 마지막으로 값을 받은 시각 */
	bool valid;
	bool has_cand;
	/* 통계(ethtool -S) */
	u64 updates;
	u64 retries;
	u64 rejects;
};

/* offset 레지스터를 읽어 state 를 갱신한다(1 ms 에 한 번). 반환값 = se_rtc_usable(). */
bool se_rtc_ofs_update(void __iomem *bar0, struct se_rtc_state *st);

/* 확인된 offset 이 있고 큰 변화 후보를 확인 중이 아닐 때만 시각을 붙인다 */
static inline bool se_rtc_usable(const struct se_rtc_state *st)
{
	return st->valid && !st->has_cand;
}

/* 아래 둘은 사용자 공간 단위 시험에서도 쓴다 */
struct se_rtc_ofs se_rtc_ofs_from_regs(u32 sh, u32 sl, u32 ns);
s64 se_rtc_ofs_diff_ns(const struct se_rtc_ofs *a, const struct se_rtc_ofs *b);

/* 수신 curtime(초, ns) + offset -> gPTP 시각. 정수 연산만 쓴다. */
ktime_t se_rtc_to_gptp(u64 raw_sec, u32 raw_ns, const struct se_rtc_ofs *ofs);

#endif /* SE_RTC_H */
