project(stbl_unit_tests LANGUAGES CXX)

MACRO(STBL_ADD_TEST Name)
    add_executable(${Name} ${Name}.cpp)
    add_dependencies(${Name} externalLest)
    target_link_libraries(${Name} libstbl)
    target_include_directories(${Name}
        PRIVATE ${STBL_ROOT}/include
        PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}
    )
    add_and_run_test(${Name} ${CMAKE_CURRENT_BINARY_DIR})
ENDMACRO(STBL_ADD_TEST)


#STBL_ADD_TEST(stbl_links_in_lists)
