cmake_minimum_required(VERSION 3.0)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake_scripts")

if (NOT DEFINED STBL_VERSION)
    set(STBL_VERSION 0.13.2)
endif()
#add_definitions(-DSTBL_VERSION=\"${STBL_VERSION}\")

set(NSBLAST_ROOT ${CMAKE_CURRENT_SOURCE_DIR})

project (stbl_project
    DESCRIPTION "Static web-site generator"
    HOMEPAGE_URL https://github.com/jgaa/stbl
    VERSION ${STBL_VERSION}
    LANGUAGES CXX)

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

find_package(cmark-gfm REQUIRED)

# Include Boost as an imported target
find_package(Boost 1.75 REQUIRED COMPONENTS
    system
    program_options
    date_time
    chrono
    log
    )

find_package(Threads REQUIRED)
find_package(JPEG REQUIRED)

configure_file(config.h.template ${CMAKE_BINARY_DIR}/generated-include/stbl/stbl_config.h)

add_subdirectory(src/libstbl)
add_subdirectory(src/mkres)
add_subdirectory(src/stbl)

if (STBL_WITH_UNIT_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()
