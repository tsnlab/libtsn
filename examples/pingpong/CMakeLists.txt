cmake_minimum_required(VERSION 3.10)

add_executable(ping ping.c common.c)
add_executable(pong pong.c common.c)
target_link_libraries(ping tsn)
target_link_libraries(pong tsn)
