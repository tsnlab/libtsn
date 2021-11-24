#include <tsn/socket.h> // TSN 패킷을 송수신 하기 위해 필요합니다

#include <arpa/inet.h>
#include <linux/if.h>
#include <net/ethernet.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/time.h>

#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "common.h"

#define TIMEOUT_SEC 1 // 타임아웃 시간

volatile __sig_atomic_t running = 1; // 시그널 처리를 위한 변수

// 시그널 처리 함수
void sigint(int signal) {
    fprintf(stderr, "Interrupted\n");
    running = 0;
}

int main(int argc, char** argv) {

    const char* iface = argv[1];  // ex) eth0
    const char* dstmac = argv[2]; // ex) 00:00:00:00:00:01

    // TSN 패킷을 송수신 하기 위한 로우소켓을 생성합니다
    int sock = tsn_sock_open(iface, VLAN_ID, VLAN_PRI, ETHERTYPE_PINGPONG);

    // 실패하면 종료합니다
    if (sock <= 0) {
        perror("socket create");
        exit(1);
    }

    // 타임아웃을 설정합니다
    struct timeval timeout = {TIMEOUT_SEC, 0};
    if (setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) < 0) {
        perror("Set socket timeout");
        return 1;
    }

    // 시그널 처리를 위한 함수를 설정합니다
    signal(SIGINT, sigint);

    // 송신용 패킷을 생성합니다
    const size_t pkt_len = sizeof(struct pingpong_pkt) + sizeof(struct ethhdr);
    uint8_t* pkt = malloc(pkt_len);
    struct ethhdr* eth = (struct ethhdr*)pkt;
    struct pingpong_pkt* payload = (struct pingpong_pkt*)(pkt + sizeof(struct ethhdr));

    // 송신용 패킷을 채웁니다
    uint8_t my_mac[ETHER_ADDR_LEN];

    // 인터페이스에서 맥 주소를 가져옵니다
    struct ifreq ifr;
    strcpy(ifr.ifr_name, iface);
    if (ioctl(sock, SIOCGIFHWADDR, &ifr) == 0) {
        memcpy(my_mac, ifr.ifr_addr.sa_data, 6);
    } else {
        printf("Failed to get mac adddr\n");
    }

    while (running) {
        tsn_recv(sock, pkt, pkt_len);

        if (ntohl(payload->type) != PKT_PING) {
            continue;
        }

        memcpy(eth->h_dest, eth->h_source, ETHER_ADDR_LEN); // 수신자와 송신자를 바꿉니다
        memcpy(eth->h_source, my_mac, ETHER_ADDR_LEN);      // 송신자를 변경합니다

        payload->type = htonl(PKT_PONG); // 타입을 변경합니다

        int sent = tsn_send(sock, pkt, pkt_len); // 패킷을 송신합니다
        if (sent < 0) {
            perror("Failed to send pkt");
        }
    }

    // 소켓을 닫습니다
    fprintf(stderr, "Closing socket\n");
    tsn_sock_close(sock);

    free(pkt); // 할당한 메모리를 해제합니다

    return 0;
}
