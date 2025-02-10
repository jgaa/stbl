project(stbl VERSION ${STBL_VERSION} LANGUAGES CXX)

set(SOURCES
    main.cpp
    )

add_executable(${PROJECT_NAME} ${SOURCES})
target_link_libraries(${PROJECT_NAME} PRIVATE libstbl ${Boost_LIBRARIES} ${JPEG_LIBRARIES} ${CMAKE_THREAD_LIBS_INIT})

target_include_directories(${PROJECT_NAME}
    PRIVATE ${STBL_ROOT}/include>
    PRIVATE ${CMAKE_BINARY_DIR}/generated-include/
    PRIVATE ${STBL_ROOT}/src/${PROJECT_NAME}
    PRIVATE ${CMAKE_CURRENT_BINARY_DIR}
)

install(TARGETS ${PROJECT_NAME} DESTINATION ${CMAKE_INSTALL_BINDIR})
