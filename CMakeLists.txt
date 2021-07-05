cmake_minimum_required(VERSION 3.10)

project(libtsn)

FILE(GLOB SRC src/*.c)

add_library(tsn STATIC ${SRC})
include_directories(include)
target_compile_options(tsn PRIVATE -Wall -Wextra -pedantic -Werror)

add_subdirectory(examples)
