cmake_minimum_required(VERSION 3.21)
project(blockwork_android LANGUAGES CXX)

if(NOT ANDROID)
    message(FATAL_ERROR "Use the Qt Android toolchain and scripts/build-android.sh")
endif()
set(CMAKE_CXX_STANDARD 17)
set(BLOCKSTITCH_DIR "" CACHE PATH "Pinned Blockstitch checkout")
set(LUCIDE_QT_DIR "" CACHE PATH "Pinned lucide-qt checkout")
set(BLOCKWORK_RUNTIME "" CACHE FILEPATH "libblockwork_android.so")
foreach(required BLOCKSTITCH_DIR LUCIDE_QT_DIR BLOCKWORK_RUNTIME)
    if(NOT EXISTS "${${required}}")
        message(FATAL_ERROR "${required} must exist")
    endif()
endforeach()
# Qt's Android kit exports Shapes as QuickShapesPrivate; it is consumed
# through QML. Support the shared manifest's older QuickShapes spelling.
set(Qt6QuickShapes_DIR "${CMAKE_CURRENT_LIST_DIR}/cmake/Qt6QuickShapes")
find_package(Qt6 6.10.3 EXACT REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QuickDialogs2 QuickShapes)
qt_standard_project_setup(REQUIRES 6.10)
include("${BLOCKSTITCH_DIR}/cmake/BlockstitchQml.cmake")
include("${CMAKE_CURRENT_LIST_DIR}/../blockwork-qt/cmake/BlockworkEditorQml.cmake")
blockstitch_add_qml_module(blockstitch)
blockwork_add_editor_qml_module(blockwork LUCIDE_QT_DIR "${LUCIDE_QT_DIR}"
    HOST_FILES "${CMAKE_CURRENT_LIST_DIR}/shims/TitleBarDrag.qml"
               "${CMAKE_CURRENT_LIST_DIR}/shims/WindowControls.qml"
               "${CMAKE_CURRENT_LIST_DIR}/shims/ScreenOverlay.qml")
qt_add_executable(blockwork-android main.cpp)
qt_add_qml_module(blockwork-android URI com.blockworked.Android VERSION 1.0
    SOURCES bridge.h bridge.cpp QML_FILES qml/Main.qml NO_LINT)
add_library(blockwork_runtime SHARED IMPORTED)
set_target_properties(blockwork_runtime PROPERTIES IMPORTED_LOCATION "${BLOCKWORK_RUNTIME}" IMPORTED_NO_SONAME TRUE)
target_link_libraries(blockwork-android PRIVATE blockwork_runtime blockworkplugin blockstitchplugin
    Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::QuickDialogs2 Qt6::QuickShapes)
target_link_options(blockwork-android PRIVATE "-Wl,-z,max-page-size=16384")
set_target_properties(blockwork-android PROPERTIES
    QT_ANDROID_PACKAGE_SOURCE_DIR "${CMAKE_CURRENT_LIST_DIR}"
    QT_ANDROID_PACKAGE_NAME "com.blockworked.blockwork"
    QT_ANDROID_MIN_SDK_VERSION 30
    QT_ANDROID_TARGET_SDK_VERSION 36
    QT_ANDROID_COMPILE_SDK_VERSION 36
    QT_ANDROID_VERSION_NAME "0.6.0"
    QT_ANDROID_VERSION_CODE 600
    QT_ANDROID_EXTRA_LIBS "${BLOCKWORK_RUNTIME}")
