cmake_minimum_required(VERSION 3.0)

include(FetchContent)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake_scripts")
option(USE_STATIC_BOOST "Link Boost statically" ON)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if (NOT DEFINED STBL_VERSION)
    set(STBL_VERSION 0.16.1)
endif()
#add_definitions(-DSTBL_VERSION=\"${STBL_VERSION}\")

set(NSBLAST_ROOT ${CMAKE_CURRENT_SOURCE_DIR})

project (stbl_project
    DESCRIPTION "Static web-site generator"
    HOMEPAGE_URL https://github.com/jgaa/stbl
    VERSION ${STBL_VERSION}
    LANGUAGES CXX)

set (STBL_ROOT ${CMAKE_CURRENT_SOURCE_DIR})

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

# Try to find an installed logfault
find_path(LOGFAULT_DIR NAMES logfault.h PATH_SUFFIXES logfault)
if (NOT LOGFAULT_DIR STREQUAL "LOGFAULT_DIR-NOTFOUND")
    message(STATUS "Using existing logfault at: ${LOGFAULT_DIR}")
    add_library(logfault INTERFACE IMPORTED)
    target_include_directories(logfault INTERFACE ${LOGFAULT_DIR})
else()
    message(STATUS "Downloading and embedding logfault (header-only)")

    FetchContent_Declare(
        logfault
        GIT_REPOSITORY "https://github.com/jgaa/logfault.git"
        GIT_TAG "master"
    )
    FetchContent_MakeAvailable(logfault)
endif()

# Include Boost as an imported target
if (USE_STATIC_BOOST)
    set(Boost_USE_STATIC_LIBS ON)
endif()
find_package(Boost 1.75 REQUIRED COMPONENTS
    system
    program_options
    date_time
    chrono
    process
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
