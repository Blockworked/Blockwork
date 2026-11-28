# Blockwork's KWin plugin: extra input seats aimed at one window each, for
# the In window block. See README.md.
cmake_minimum_required(VERSION 3.24)
project(blockwork_kwin VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui)
include(GNUInstallDirs)
find_package(PkgConfig REQUIRED)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon)
pkg_check_modules(WAYLAND_SERVER REQUIRED IMPORTED_TARGET wayland-server)
find_program(WAYLAND_SCANNER wayland-scanner REQUIRED)

find_path(KWIN_INCLUDE_DIR plugin.h PATH_SUFFIXES kwin REQUIRED)
find_library(KWIN_LIBRARY NAMES kwin REQUIRED)

# KWin only loads a plugin built against its exact version.
file(READ "${KWIN_INCLUDE_DIR}/config-kwin.h" KWIN_CONFIG)
string(REGEX MATCH "#define KWIN_PLUGIN_VERSION_STRING \"([0-9.]+)\"" _ "${KWIN_CONFIG}")
if(NOT CMAKE_MATCH_1)
    message(FATAL_ERROR "config-kwin.h has no KWIN_PLUGIN_VERSION_STRING: is KWin 6 installed?")
endif()
message(STATUS "Building for KWin ${CMAKE_MATCH_1}")

set(PROTOCOL "${CMAKE_CURRENT_SOURCE_DIR}/../blockwork-core/protocols/blockwork-seat-host-v1.xml")
set(GENERATED "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${GENERATED}")
add_custom_command(
    OUTPUT "${GENERATED}/blockwork-seat-host-v1-server.h" "${GENERATED}/blockwork-seat-host-v1.c"
    COMMAND ${WAYLAND_SCANNER} server-header ${PROTOCOL} "${GENERATED}/blockwork-seat-host-v1-server.h"
    COMMAND ${WAYLAND_SCANNER} private-code ${PROTOCOL} "${GENERATED}/blockwork-seat-host-v1.c"
    DEPENDS ${PROTOCOL} VERBATIM)

add_library(blockworkseathost MODULE
    main.cpp
    seathost.cpp
    seathost.h
    "${GENERATED}/blockwork-seat-host-v1-server.h"
    "${GENERATED}/blockwork-seat-host-v1.c")
set_target_properties(blockworkseathost PROPERTIES PREFIX "")
target_compile_definitions(blockworkseathost PRIVATE BLOCKWORK_KWIN_BUILD="${PROJECT_VERSION}")
target_include_directories(blockworkseathost PRIVATE ${KWIN_INCLUDE_DIR} "${GENERATED}")
target_link_libraries(blockworkseathost PRIVATE
    ${KWIN_LIBRARY} Qt6::Core Qt6::Gui PkgConfig::XKBCOMMON PkgConfig::WAYLAND_SERVER)

install(TARGETS blockworkseathost
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}/qt6/plugins/kwin/plugins)
