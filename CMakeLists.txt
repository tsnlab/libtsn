cmake_minimum_required(VERSION 3.10)

project(tsn)

option(BUILD_EXAMPLES "Build examples" OFF)

set(PUBLIC_INCLUDE_DIRECTORIES include/)
set(PRIVATE_INCLUDE_DIRECTORIES src/)

file(GLOB_RECURSE SOURCE_FILES "src/*.c" "src/*.h")
file(GLOB_RECURSE INCLUDE_FILES "include/*.h")

add_library(${PROJECT_NAME} STATIC ${INCLUDE_FILES} ${SOURCE_FILES})
target_include_directories(${PROJECT_NAME} PUBLIC ${PUBLIC_INCLUDE_DIRECTORIES})
target_include_directories(${PROJECT_NAME} PRIVATE ${PRIVATE_INCLUDE_DIRECTORIES})
target_compile_options(tsn PRIVATE -Wall -Wextra -pedantic -Werror)

set(UCD_RUNTIME_NAME "tsnucd")

find_program(PYTHON3 "python3")

if (PYTHON3)
    FILE(GLOB PYTSN_SRC pytsn/*.py)
    add_custom_command(
        OUTPUT ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME}
        COMMAND ${PYTHON3}
        ARGS -m zipapp -c -o ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME} ${PROJECT_SOURCE_DIR}/pytsn -p "/usr/bin/env -S python3 -u"
        DEPENDS ${PYTSN_SRC}
        )
    add_custom_target(
        target
        ALL
        COMMENT make ${UCD_RUNTIME_NAME} executable
        DEPENDS ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME}
        VERBATIM
        )
endif(PYTHON3)

if (BUILD_EXAMPLES)
    add_subdirectory(examples)
endif (BUILD_EXAMPLES)

install(
    TARGETS ${PROJECT_NAME}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    COMPONENT library
)
install(
    DIRECTORY "${PUBLIC_INCLUDE_DIRECTORIES}"
    DESTINATION include
)
install(
    FILES ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME}
    DESTINATION ${CMAKE_INSTALL_PREFIX}/bin/
)
