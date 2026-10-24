#!/bin/bash
# check_nofpu.sh - xdma.ko 에 FPU/SIMD 레지스터 사용이 없는지 검사한다.
#
# 배경(2026-10-07): x86 빌드에 -msse2 를 넣었을 때 컴파일러가 IRQ·softirq·start_xmit
# 코드에서 SSE 레지스터를 썼다. 커널은 이 상태를 저장하지 않는다. 그래서 사용자
# 프로세스의 레지스터가 깨졌다(segfault). 이 검사는 같은 결함의 재발을 막는다.
#
# 사용법: check_nofpu.sh [모듈 경로]
#   인자가 없으면 xdma/xdma.ko(또는 .zst/.xz/.gz)를 검사한다(DKMS POST_BUILD 에서 build 디렉터리 기준).
#   .ko.zst, .ko.xz, .ko.gz 도 받는다.
#
# 판정:
#   x86_64: %xmm, %ymm, %zmm 레지스터가 하나라도 있으면 실패(종료 코드 1).
#   그 밖의 아키텍처(arm64 등): 검사하지 않고 통과한다. 메시지만 출력한다.
#     arm64 커널은 -mgeneral-regs-only 로 빌드한다. Makefile 은 이 플래그를 끄지 않는다.
# 종료 코드: 0 통과, 1 SIMD 사용 발견, 2 사용 오류(파일 없음, objdump 없음 등).
set -euo pipefail

ko="${1:-}"
if [ -z "$ko" ]; then
	# DKMS POST_BUILD: dkms 3.x 는 빌드 직후 서명·압축해 xdma.ko.zst 만 남긴다(2026-10-12 dkms 3.2.2 실측).
	for c in xdma/xdma.ko xdma/xdma.ko.zst xdma/xdma.ko.xz xdma/xdma.ko.gz; do
		[ -f "$c" ] && ko="$c" && break
	done
	ko="${ko:-xdma/xdma.ko}"
fi

if [ ! -f "$ko" ]; then
	echo "check_nofpu: 모듈 파일이 없습니다: $ko" >&2
	exit 2
fi
if ! command -v objdump >/dev/null 2>&1; then
	echo "check_nofpu: objdump 가 없습니다. binutils 를 설치하십시오." >&2
	exit 2
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

# 압축된 모듈을 풀어서 검사한다.
case "$ko" in
*.zst)
	zstd -q -d -c "$ko" >"$tmpdir/m.ko"
	obj="$tmpdir/m.ko"
	;;
*.xz)
	xz -d -c "$ko" >"$tmpdir/m.ko"
	obj="$tmpdir/m.ko"
	;;
*.gz)
	gzip -d -c "$ko" >"$tmpdir/m.ko"
	obj="$tmpdir/m.ko"
	;;
*)
	obj="$ko"
	;;
esac

# 실행 호스트가 아니라 모듈 파일의 아키텍처로 판정한다.
arch="$(objdump -f "$obj" | sed -n 's/^architecture: \([^,]*\).*/\1/p')"

case "$arch" in
i386:x86-64*)
	objdump -d "$obj" >"$tmpdir/dis.txt"
	if grep -Eq '%[xyz]mm[0-9]+' "$tmpdir/dis.txt"; then
		count="$(grep -Ec '%[xyz]mm[0-9]+' "$tmpdir/dis.txt")"
		echo "check_nofpu: 실패. $ko 에 SSE/AVX 레지스터 명령이 ${count} 개 있습니다." >&2
		echo "check_nofpu: 처음 20 개:" >&2
		grep -E '%[xyz]mm[0-9]+' "$tmpdir/dis.txt" | head -n 20 >&2
		echo "check_nofpu: 빌드 플래그에 -msse2 등 FPU/SIMD 옵션이 들어갔는지 확인하십시오." >&2
		exit 1
	fi
	echo "check_nofpu: 통과. $ko ($arch) 에 xmm/ymm/zmm 레지스터 사용이 없습니다."
	;;
*)
	echo "check_nofpu: $ko 의 아키텍처는 ${arch:-알 수 없음} 입니다. x86_64 가 아니므로 검사하지 않습니다."
	;;
esac
exit 0
