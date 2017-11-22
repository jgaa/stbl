project(libstbl VERSION 1.10 LANGUAGES CXX)

include_directories(${STBL_ROOT}/externals/cpp-markdown/src/)
include_directories(${STBL_ROOT}/3rdparty/gil_extensions)

add_custom_command(
    COMMAND mkres stbl embedded_templates_ templates_res.cpp templates_res.h ${STBL_ROOT}/examples/default/templates/*.html
    DEPENDS ${STBL_ROOT}/examples/default/templates/*.html mkres
    OUTPUT templates_res.cpp templates_res.h
    COMMENT "Embedding templates..."
    )


set(SOURCES
    ContentManagerImpl.cpp
    DirectoryScannerImpl.cpp
    SeriesImpl.cpp
    ArticleImpl.cpp
    ContentImpl.cpp
    PageImpl.cpp
    logging.cpp
    HeaderParserImpl.cpp
    ImageImpl.cpp
    ImageMgrImpl.cpp
    utility.cpp
    templates_res.cpp
    ${STBL_ROOT}/externals/cpp-markdown/src/markdown-tokens.cpp
    ${STBL_ROOT}/externals/cpp-markdown/src/markdown.cpp
    )

add_library(libstbl ${SOURCES})
set_property(TARGET libstbl PROPERTY CXX_STANDARD 14)
target_include_directories(libstbl
    PUBLIC $<BUILD_INTERFACE:${STBL_ROOT}/include> $<INSTALL_INTERFACE:include>
    PRIVATE ${STBL_ROOT}/src/libstbl
    PRIVATE ${CMAKE_CURRENT_BINARY_DIR}
)
target_link_libraries(libstbl PUBLIC ${Boost_LIBRARIES} ${JPEG_LIBRARIES})


# if (WIN32)
#     set(SOURCES ${ACTUAL_SOURCES} ${HEADERS} ${RESFILES})
# else()
#     set(SOURCES ${ACTUAL_SOURCES})
# endif()
#
# add_library(libstbl ${SOURCES})
# target_include_directories(libstbl PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
# set_target_properties(libstbl PROPERTIES DEBUG_OUTPUT_NAME stblD)
# target_link_libraries(libstbl ${DEFAULT_LIBRARIES})
