cmake_minimum_required(VERSION 3.19)
project(BlockworkWindowLibrary LANGUAGES CXX)
include(FetchContent)

if(POLICY CMP0135)
    cmake_policy(SET CMP0135 NEW)
endif()
if(POLICY CMP0170)
    cmake_policy(SET CMP0170 NEW)
endif()

set(QWINDOWKIT_BUILD_STATIC ON CACHE BOOL "" FORCE)
set(QWINDOWKIT_BUILD_QUICK ON CACHE BOOL "" FORCE)
set(QWINDOWKIT_BUILD_WIDGETS OFF CACHE BOOL "" FORCE)
set(QWINDOWKIT_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(QWINDOWKIT_ENABLE_STYLE_AGENT OFF CACHE BOOL "" FORCE)

# Cache the upstream source in the build directory, outside the checkout.
FetchContent_Declare(QWindowKit
    URL https://codeload.github.com/stdware/qwindowkit/tar.gz/2072958b04820e9e7629f6c40068dee0ce0e9915
    URL_HASH SHA256=82cf5292b7f39729f2d40f63cee340dd77aea3c402d04df735c23b76f98c822c
    TLS_VERIFY ON
)
FetchContent_MakeAvailable(QWindowKit)
