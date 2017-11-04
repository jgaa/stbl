
project (stbl)

set(ACTUAL_SOURCES
    main.cpp
    )

set(HEADERS
    ${STBL_ROOT_DIR}/include/stbl/BlogPost.h
    )


if (WIN32)
    set(SOURCES ${ACTUAL_SOURCES} ${HEADERS} ${RESFILES})
else()
    set(SOURCES ${ACTUAL_SOURCES})
endif()


add_executable(stbl ${SOURCES})
set_target_properties(stbl PROPERTIES DEBUG_OUTPUT_NAME stblD)
target_link_libraries(stbl
    libstbl
    ${DEFAULT_LIBRARIES}
    )

install(TARGETS stbl DESTINATION bin)
