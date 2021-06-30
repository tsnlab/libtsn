cmake_minimum_required(VERSION 3.10)

file(GLOB subdir *)
foreach(dir ${subdir})
    if(IS_DIRECTORY ${dir})
        add_subdirectory(${dir})
    endif()
endforeach()
