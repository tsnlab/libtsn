#include "common.h"

#include <stdio.h>

bool strtomac(uint8_t* mac, const char* str) {
    int tmp[6];
    int res = sscanf(str, "%02x:%02x:%02x:%02x:%02x:%02x", &tmp[0], &tmp[1], &tmp[2], &tmp[3], &tmp[4], &tmp[5]);

    for (int i = 0; i < 6; i += 1) {
        mac[i] = tmp[i];
    }

    return res == 6;
}

bool mactostr(char* str, const uint8_t* mac) {
    snprintf(str, 18, "%02x:%02x:%02x:%02x:%02x:%02x", mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    return true;
}
