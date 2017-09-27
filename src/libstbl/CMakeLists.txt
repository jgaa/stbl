project(libstbl)

set(ACTUAL_SOURCES
    BlogPost.cpp
    )

set(HEADERS
    ${STBL_ROOT_DIR}/include/stbl/BlogPost.h
    )


if (WIN32)
    set(SOURCES ${ACTUAL_SOURCES} ${HEADERS} ${RESFILES})
else()
    set(SOURCES ${ACTUAL_SOURCES})
endif()

add_library(libstbl ${SOURCES})
set_target_properties(libstbl PROPERTIES DEBUG_OUTPUT_NAME stblD)
target_link_libraries(libstbl ${DEFAULT_LIBRARIES})
