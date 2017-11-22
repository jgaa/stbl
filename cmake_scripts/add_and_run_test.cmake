macro (ADD_AND_RUN_TEST testname cwd)
add_test(NAME ${testname} COMMAND ${testname})
if (STBL_AUTORUN_UNIT_TESTS)
    message(STATUS "Adding test for auto-run:  ${testname}")
    add_custom_command(
        TARGET ${testname}
        POST_BUILD
        COMMENT "running ${testname}"
        WORKING_DIRECTORY ${cwd}
        COMMAND ${CMAKE_CTEST_COMMAND} -C $<CONFIGURATION> -R "${testname}"  --output-on-failure
    )
endif()
endmacro()

