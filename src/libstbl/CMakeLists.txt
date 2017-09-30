project(libstbl)

set(ACTUAL_SOURCES
    ContentManagerImpl.cpp
    DirectoryScannerImpl.cpp
    SeriesImpl.cpp
    ArticleImpl.cpp
    ContentImpl.cpp
    PageImpl.cpp
    logging.cpp
    HeaderParserImpl.cpp
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
    )

if (WIN32)
    set(SOURCES ${ACTUAL_SOURCES} ${HEADERS} ${RESFILES})
else()
    set(SOURCES ${ACTUAL_SOURCES})
endif()

add_library(libstbl ${SOURCES})
set_target_properties(libstbl PROPERTIES DEBUG_OUTPUT_NAME stblD)
target_link_libraries(libstbl ${DEFAULT_LIBRARIES})
