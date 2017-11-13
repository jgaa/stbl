project(libstbl)

include_directories(${STBL_ROOT_DIR}/externals/cpp-markdown/src/)
include_directories(${STBL_ROOT_DIR}/3rdparty/gil_extensions)

add_custom_command(
    COMMAND mkres stbl embedded_templates_ templates_res.cpp templates_res.h ${STBL_ROOT_DIR}/tests/testsite/templates/*.html
    DEPENDS ${STBL_ROOT_DIR}/tests/testsite/templates/*.html mkres
    OUTPUT templates_res.cpp templates_res.h
    COMMENT "Embedding templates..."
    )

set(ACTUAL_SOURCES
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
    ${STBL_ROOT_DIR}/externals/cpp-markdown/src/markdown-tokens.cpp
    ${STBL_ROOT_DIR}/externals/cpp-markdown/src/markdown.cpp
    )

set(HEADERS
    ${STBL_ROOT_DIR}/include/stbl/Node.h
    ${STBL_ROOT_DIR}/include/stbl/Article.h
    ${STBL_ROOT_DIR}/include/stbl/Series.h
    ${STBL_ROOT_DIR}/include/stbl/Content.h
    ${STBL_ROOT_DIR}/include/stbl/Page.h
    ${STBL_ROOT_DIR}/include/stbl/stbl.h
    ${STBL_ROOT_DIR}/include/stbl/Options.h
    ${STBL_ROOT_DIR}/include/stbl/ContentManager.h
    templates_res.h
    )

if (WIN32)
    set(SOURCES ${ACTUAL_SOURCES} ${HEADERS} ${RESFILES})
else()
    set(SOURCES ${ACTUAL_SOURCES})
endif()

add_library(libstbl ${SOURCES})
target_include_directories(libstbl PRIVATE ${CMAKE_CURRENT_BINARY_DIR})
set_target_properties(libstbl PROPERTIES DEBUG_OUTPUT_NAME stblD)
target_link_libraries(libstbl ${DEFAULT_LIBRARIES})
