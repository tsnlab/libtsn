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

if (CMAKE_VERSION VERSION_LESS 3.12)
    find_program(PYTHON3 "python3")
    set(Python3_EXECUTABLE "${PYTHON3}")
    set(PYTHON3_FOUND TRUE)
    # TODO: check python version
else ()
    find_package(Python3 3.5 REQUIRED COMPONENTS Interpreter)
endif (CMAKE_VERSION VERSION_LESS 3.12)

if (PYTHON3_FOUND)
    FILE(GLOB PYTSN_SRC pytsn/*.py)
    if(PYTHON3_VERSION_MINOR GREATER_EQUAL 7)
        set(PYZIPAPP_OPT_COMPRESS "-c")
    endif(PYTHON3_VERSION_MINOR GREATER_EQUAL 7)
    add_custom_command(
        OUTPUT ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME}
        COMMAND ${Python3_EXECUTABLE}
        ARGS -m zipapp ${PYZIPAPP_OPT_COMPRESS} -o ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME} ${PROJECT_SOURCE_DIR}/pytsn -p "/usr/bin/env -S python3 -u"
        DEPENDS ${PYTSN_SRC}
        )
    add_custom_target(
        target
        ALL
        COMMENT make ${UCD_RUNTIME_NAME} executable
        DEPENDS ${PROJECT_BINARY_DIR}/${UCD_RUNTIME_NAME}
        VERBATIM
        )
endif(PYTHON3_FOUND)

if (BUILD_EXAMPLES)
    add_subdirectory(examples)
endif (BUILD_EXAMPLES)

include(GNUInstallDirs)

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
