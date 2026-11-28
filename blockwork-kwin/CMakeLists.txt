# Blockwork's KWin plugin: extra input seats aimed at one window each, for
# the In window block. See README.md.
cmake_minimum_required(VERSION 3.24)
project(blockwork_kwin VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# KWin::kwin links these but its config doesn't find them.
find_package(Qt6 REQUIRED COMPONENTS Core DBus Gui Quick Widgets)
# KWin's config brings the KF6, epoxy, libdrm and Vulkan headers its own
# headers include.
find_package(KWin REQUIRED)
include(GNUInstallDirs)
find_package(PkgConfig REQUIRED)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon)
pkg_check_modules(WAYLAND_SERVER REQUIRED IMPORTED_TARGET wayland-server)
find_program(WAYLAND_SCANNER wayland-scanner REQUIRED)
message(STATUS "Building for KWin ${KWin_VERSION}")

set(PROTOCOL "${CMAKE_CURRENT_SOURCE_DIR}/../blockwork-core/protocols/blockwork-seat-host-v1.xml")
set(GENERATED "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${GENERATED}")
add_custom_command(
    OUTPUT "${GENERATED}/blockwork-seat-host-v1-server.h" "${GENERATED}/blockwork-seat-host-v1.c"
    COMMAND ${WAYLAND_SCANNER} server-header ${PROTOCOL} "${GENERATED}/blockwork-seat-host-v1-server.h"
    COMMAND ${WAYLAND_SCANNER} private-code ${PROTOCOL} "${GENERATED}/blockwork-seat-host-v1.c"
    DEPENDS ${PROTOCOL} VERBATIM)
# From wayland-protocols, kept here since it's only a build dependency there.
set(CONSTRAINTS "${CMAKE_CURRENT_SOURCE_DIR}/pointer-constraints-unstable-v1.xml")
add_custom_command(
    OUTPUT "${GENERATED}/pointer-constraints-unstable-v1-server.h" "${GENERATED}/pointer-constraints-unstable-v1.c"
    COMMAND ${WAYLAND_SCANNER} server-header ${CONSTRAINTS} "${GENERATED}/pointer-constraints-unstable-v1-server.h"
    COMMAND ${WAYLAND_SCANNER} private-code ${CONSTRAINTS} "${GENERATED}/pointer-constraints-unstable-v1.c"
    DEPENDS ${CONSTRAINTS} VERBATIM)

add_library(blockworkseathost MODULE
    main.cpp
    seathost.cpp
    seathost.h
    "${GENERATED}/blockwork-seat-host-v1-server.h"
    "${GENERATED}/blockwork-seat-host-v1.c"
    "${GENERATED}/pointer-constraints-unstable-v1-server.h"
    "${GENERATED}/pointer-constraints-unstable-v1.c")
set_target_properties(blockworkseathost PROPERTIES PREFIX "")
target_compile_definitions(blockworkseathost PRIVATE BLOCKWORK_KWIN_BUILD="${PROJECT_VERSION}")
target_include_directories(blockworkseathost PRIVATE "${GENERATED}")
target_link_libraries(blockworkseathost PRIVATE
    KWin::kwin Qt6::Core Qt6::Gui PkgConfig::XKBCOMMON PkgConfig::WAYLAND_SERVER)

install(TARGETS blockworkseathost
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}/qt6/plugins/kwin/plugins)
