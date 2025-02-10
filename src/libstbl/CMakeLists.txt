project(libstbl VERSION ${STBL_VERSION} LANGUAGES CXX)

#include_directories(${STBL_ROOT}/3rdparty/gil_extensions)

file(GLOB TEMPLATE_SOURCES ${STBL_ROOT}/examples/bootstrap/templates/*.html)

add_custom_command(
    COMMAND mkres stbl embedded_templates_ templates_res.cpp templates_res.h ${TEMPLATE_SOURCES}
    DEPENDS mkres ${TEMPLATE_SOURCES}
    OUTPUT templates_res.cpp templates_res.h
    COMMENT "Embedding templates..."
    )

file(GLOB ARTIFACT_SOURCES ${STBL_ROOT}/examples/bootstrap/artifacts/*)

add_custom_command(
    COMMAND mkres stbl embedded_artifacts_ artifacts_res.cpp artifacts_res.h ${ARTIFACT_SOURCES}
    DEPENDS mkres ${ARTIFACT_SOURCES}
    OUTPUT artifacts_res.cpp artifacts_res.h
    COMMENT "Embedding artifacts..."
    )

file(GLOB CONFIG_SOURCES ${STBL_ROOT}/examples/bootstrap/*.conf)

add_custom_command(
    COMMAND mkres stbl embedded_config_ config_res.cpp config_res.h ${CONFIG_SOURCES}
    DEPENDS mkres ${CONFIG_SOURCES}
    OUTPUT config_res.cpp config_res.h
    COMMENT "Embedding config..."
    )

file(GLOB ARTICLES_SOURCES ${STBL_ROOT}/examples/bootstrap/articles/*.md)

add_custom_command(
    COMMAND mkres stbl embedded_articles_ articles_res.cpp articles_res.h ${ARTICLES_SOURCES}
    DEPENDS mkres ${ARTICLES_SOURCES}
    OUTPUT articles_res.cpp articles_res.h
    COMMENT "Embedding articles..."
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
    BootstrapImpl.cpp
    SitemapImpl.cpp
    templates_res.cpp
    artifacts_res.cpp
    config_res.cpp
    articles_res.cpp
    )

add_library(libstbl ${SOURCES})
target_include_directories(libstbl
    PUBLIC $<BUILD_INTERFACE:${STBL_ROOT}/include> $<INSTALL_INTERFACE:include>
    PRIVATE ${STBL_ROOT}/src/libstbl
    PRIVATE ${CMAKE_CURRENT_BINARY_DIR}
    PRIVATE ${CMAKE_BINARY_DIR}/generated-include
    PRIVATE ${cmark-gfm_INCLUDE_DIRS}
)
target_link_libraries(libstbl PUBLIC ${cmark-gfm_LIBRARIES} ${Boost_LIBRARIES} ${JPEG_LIBRARIES} logfault)
