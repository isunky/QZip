cmake_minimum_required(VERSION 3.21)
project(qzip_shell LANGUAGES CXX)

add_library(qzip-shell SHARED qzip_shell.cpp)
target_compile_features(qzip-shell PRIVATE cxx_std_17)
target_compile_definitions(qzip-shell PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(qzip-shell PRIVATE /utf-8)
target_link_libraries(qzip-shell PRIVATE ole32 shell32)
set_target_properties(qzip-shell PROPERTIES OUTPUT_NAME "qzip-shell")
