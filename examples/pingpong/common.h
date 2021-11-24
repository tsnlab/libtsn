#include <stdbool.h>
#include <stdint.h>

#define VLAN_ID 10
#define VLAN_PRI 3 // VLAN PCP
#define ETHERTYPE_PINGPONG 0xdead

struct pingpong_pkt {
    uint32_t id;
    uint32_t type;
    uint8_t data[100];
};

enum pkt_type {
    PKT_PING,
    PKT_PONG,
};

bool strtomac(uint8_t* mac, const char* str);
bool mactostr(char* str, const uint8_t* mac);
