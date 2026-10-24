/* host_mac.h - 호스트 netdev MAC 계산(커널 드라이버와 사용자 공간 단위 시험이 같이 씀).
 *
 * 씨앗(seed) = 보드 DNA ^ HOST_MAC_DNA_SALT, 또는 DNA 를 못 읽으면 machine-id 해시와 인식 순서.
 * splitmix64 로 섞은 값의 하위 6 바이트를 쓰고, 첫 바이트를 유니캐스트(I/G=0) + 로컬 관리(U/L=1)로 맞춘다.
 * 보드 펌웨어가 만드는 보드 포트 MAC(02:xx)과 같은 규칙이다. 전역 OUI 와 겹치지 않고 멀티캐스트가 되지 않는다.
 */
#ifndef HOST_MAC_H
#define HOST_MAC_H

#ifdef __KERNEL__
#include <linux/types.h>
#define HOST_MAC_U64(x) U64_C(x)
#else
#include <stdint.h>
#define HOST_MAC_U64(x) UINT64_C(x)
#endif

#define HOST_MAC_DNA_SALT	HOST_MAC_U64(0x484f53544d414321)	/* 'HOSTMAC!': 보드 포트 MAC 과 다르게 */

/* splitmix64(seed ^ num). 예전 드라이버의 hash(hostid, num) 과 같은 식. */
static inline uint64_t host_mac_hash(uint64_t seed, uint64_t num)
{
	uint64_t h = seed;

	h = ((h >> 30) ^ (h ^ num)) * HOST_MAC_U64(0xbf58476d1ce4e5b9);
	h = ((h >> 27) ^ h) * HOST_MAC_U64(0x94d049bb133111eb);
	h = (h >> 31) ^ h;
	return h;
}

/* h 의 하위 6 바이트 + 포트 번호(마지막 바이트에 더함) -> mac. 첫 바이트 = (b0 & 0xFC) | 0x02 */
static inline void host_mac_compose(uint64_t h, int port_id, unsigned char *mac)
{
	int i;

	for (i = 0; i < 6; i++)
		mac[i] = (h >> (i * 8)) & 0xFF;
	mac[5] += port_id;
	mac[0] = (mac[0] & 0xFC) | 0x02;
}

#endif
