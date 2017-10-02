project (tests)

include_directories(
    ${STBL_ROOT_DIR}/externals/lest/include
    ${STBL_ROOT_DIR}/src/libstbl
    )

macro (ADD_AND_RUN_UNITTEST testname targetname)
add_test(NAME ${testname} COMMAND ${targetname})
if (STBL_AUTORUN_UNIT_TESTS)
    add_custom_command(
        TARGET ${targetname}
        POST_BUILD
        COMMENT "running ${testname}"
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
        COMMAND ${CMAKE_CTEST_COMMAND} -C $<CONFIGURATION> -R "${testname}"  --output-on-failure
    )
endif()
endmacro()

