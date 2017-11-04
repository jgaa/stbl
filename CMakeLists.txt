cmake_minimum_required(VERSION 3.0)
project (stbl-all)

if (NOT DEFINED STBL_WITH_UNIT_TESTS)
    option(STBL_WITH_UNIT_TESTS "Enable Unit Testing" ON)
endif()

if (NOT DEFINED STBL_AUTORUN_UNIT_TESTS)
    option(STBL_AUTORUN_UNIT_TESTS "Run Unit Tests automatically after build" OFF)
endif()

if (NOT DEFINED STBL_ROOT_DIR)
    set(STBL_ROOT_DIR ${CMAKE_CURRENT_SOURCE_DIR})
endif()


find_package(Boost REQUIRED COMPONENTS
        system
        program_options
        filesystem
        date_time
        chrono
        log
    )

if (WIN32)
    link_directories(${Boost_LIBRARY_DIRS})
else()
    add_definitions(-DBOOST_ALL_DYN_LINK=1 -std=c++14)
endif()

set(DEFAULT_LIBRARIES
    ${THREADLIBS}
    ${Boost_LIBRARIES}
    )

include_directories(${STBL_ROOT_DIR}/include)
include_directories(${Boost_INCLUDE_DIRS})


add_subdirectory(src)

if (STBL_WITH_UNIT_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

