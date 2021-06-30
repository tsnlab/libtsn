cmake_minimum_required(VERSION 3.10)

add_executable(latency latency.c)
target_link_libraries(latency tsn)
