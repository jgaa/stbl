project(stbl_unit_tests LANGUAGES CXX)

MACRO(WFDE_ADD_TEST Name Source)
    add_executable(${Name} ${Source})
    add_dependencies(${Name} externalLest)
    target_link_libraries(${Name} wfde)
    target_include_directories(${Name}
        PRIVATE ${STBL_ROOT_DIR}/include>
        PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}
    )
    add_and_run_test(${Name} ${CMAKE_CURRENT_BINARY_DIR})
ENDMACRO(WFDE_ADD_TEST)


