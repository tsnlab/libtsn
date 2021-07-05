cmake_minimum_required(VERSION 3.10)

project(libtsn)

option(BUILD_EXAMPLES "Build examples" OFF)

FILE(GLOB SRC src/*.c)

add_library(tsn STATIC ${SRC})
include_directories(include)
target_compile_options(tsn PRIVATE -Wall -Wextra -pedantic -Werror)

if (BUILD_EXAMPLES)
    add_subdirectory(examples)
endif (BUILD_EXAMPLES)
