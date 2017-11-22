project(mkres VERSION 1.0 LANGUAGES CXX)

add_executable(mkres main.cpp)
target_link_libraries(mkres ${Boost_LIBRARIES})
