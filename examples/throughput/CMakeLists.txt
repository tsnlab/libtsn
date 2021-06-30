cmake_minimum_required(VERSION 3.10)

add_executable(throughput throughput.c)
target_link_libraries(throughput tsn pthread)
