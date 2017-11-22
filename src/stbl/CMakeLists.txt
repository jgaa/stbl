project(stbl VERSION 0.10 LANGUAGES CXX)

set(SOURCES
    main.cpp
    )

add_executable(${PROJECT_NAME} ${SOURCES})
set_property(TARGET ${PROJECT_NAME} PROPERTY CXX_STANDARD 14)
target_link_libraries(${PROJECT_NAME} PRIVATE libstbl ${Boost_LIBRARIES} ${JPEG_LIBRARIES})

target_include_directories(${PROJECT_NAME}
    PRIVATE ${STBL_ROOT}/include>
    PRIVATE ${CMAKE_BINARY_DIR}/generated-include/
    PRIVATE ${STBL_ROOT}/src/${PROJECT_NAME}
)

install(TARGETS ${PROJECT_NAME} DESTINATION ${CMAKE_INSTALL_BINDIR})

configure_file(config.h.template ${CMAKE_BINARY_DIR}/generated-include/stbl/stbl_config.h)
