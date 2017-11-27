cmake_minimum_required(VERSION 3.0)
project (stbl_project VERSION 0.11 LANGUAGES CXX)

set (STBL_ROOT ${CMAKE_CURRENT_SOURCE_DIR})
add_definitions(-DBOOST_ALL_DYN_LINK=1)

if (NOT DEFINED STBL_WITH_UNIT_TESTS)
    option(STBL_WITH_UNIT_TESTS "Enable Unit Testing" ON)
    if (NOT DEFINED STBL_AUTORUN_UNIT_TESTS)
        option(STBL_AUTORUN_UNIT_TESTS "Run unit tests during build" OFF)
    endif()
endif()

message(STATUS "Using ${CMAKE_CXX_COMPILER}")

include(cmake_scripts/external-projects.cmake)
include(cmake_scripts/add_and_run_test.cmake)

# Include Boost as an imported target
find_package(Boost 1.50 REQUIRED COMPONENTS
    system
    program_options
    filesystem
    date_time
    chrono
    log
    )

find_package(JPEG REQUIRED)

add_subdirectory(src/libstbl)
add_subdirectory(src/mkres)
add_subdirectory(src/stbl)

if (STBL_WITH_UNIT_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()
