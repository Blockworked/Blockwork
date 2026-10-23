# CMake entry point for embedding Blockwork's editor (EditorPage and what it
# reaches) as the com.blockworked.Blockwork QML module, e.g. in the website's
# WebAssembly demo. It reads qml/module.json, the manifest build.rs builds the
# app from, and checks there that the editor group is complete.
#
#   include(<Blockwork>/blockwork-qt/cmake/BlockworkEditorQml.cmake)
#   blockwork_add_editor_qml_module(blockwork      # STATIC; link blockworkplugin
#       LUCIDE_QT_DIR <lucide-qt checkout>
#       HOST_FILES <one file per `host_provided` name>)
#
# The host supplies its own TitleBarDrag/WindowControls/ScreenOverlay (the
# desktop ones need the app's window and screen capture). The editor also
# needs com.blockworked.Blockstitch and a Qt with QuickControls2/QuickDialogs2.
cmake_minimum_required(VERSION 3.21)

set(BLOCKWORK_QT_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." CACHE INTERNAL "")
cmake_path(NORMAL_PATH BLOCKWORK_QT_ROOT)

function(_blockwork_json_list json out)
    string(JSON count LENGTH "${json}" ${ARGN})
    set(items)
    if(count GREATER 0)
        math(EXPR last "${count} - 1")
        foreach(i RANGE ${last})
            string(JSON item GET "${json}" ${ARGN} ${i})
            list(APPEND items "${item}")
        endforeach()
    endif()
    set(${out} ${items} PARENT_SCOPE)
endfunction()

# Adds files to `target` as one resource under `prefix`, each by its own name.
function(_blockwork_add_resource target name prefix)
    foreach(path ${ARGN})
        if(NOT EXISTS "${path}")
            message(FATAL_ERROR "Blockwork editor resource ${path} does not exist")
        endif()
        get_filename_component(file "${path}" NAME)
        set_source_files_properties("${path}" PROPERTIES QT_RESOURCE_ALIAS "${file}")
    endforeach()
    qt_add_resources(${target} "${name}" PREFIX "${prefix}" FILES ${ARGN})
endfunction()

function(blockwork_add_editor_qml_module target)
    cmake_parse_arguments(PARSE_ARGV 1 arg "" "LUCIDE_QT_DIR" "HOST_FILES")
    if(NOT EXISTS "${arg_LUCIDE_QT_DIR}/qml/Lucide")
        message(FATAL_ERROR "blockwork_add_editor_qml_module: LUCIDE_QT_DIR is not a lucide-qt checkout")
    endif()
    set(qml "${BLOCKWORK_QT_ROOT}/qml")
    set(manifest "${qml}/module.json")
    file(READ "${manifest}" json)
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${manifest}")
    string(JSON uri GET "${json}" uri)
    string(JSON version GET "${json}" version)
    _blockwork_json_list("${json}" editor editor)
    _blockwork_json_list("${json}" host_provided host_provided)
    _blockwork_json_list("${json}" singletons singletons)
    _blockwork_json_list("${json}" qt_modules qt_modules)
    _blockwork_json_list("${json}" lucide_files lucide files)
    _blockwork_json_list("${json}" lucide_singletons lucide singletons)

    # The host's replacement for every host-provided file, and nothing else.
    set(given)
    foreach(path ${arg_HOST_FILES})
        get_filename_component(name "${path}" NAME)
        list(APPEND given "${name}")
    endforeach()
    set(missing ${host_provided})
    list(REMOVE_ITEM missing ${given})
    set(extra ${given})
    list(REMOVE_ITEM extra ${host_provided})
    if(missing OR extra)
        message(FATAL_ERROR "blockwork_add_editor_qml_module: HOST_FILES must replace exactly [${host_provided}]; missing [${missing}], unexpected [${extra}]")
    endif()

    find_package(Qt6 6.10 REQUIRED COMPONENTS ${qt_modules})
    set(paths ${arg_HOST_FILES})
    foreach(name ${editor})
        set(path "${qml}/${name}")
        if(NOT EXISTS "${path}")
            message(FATAL_ERROR "${manifest} lists ${name}, which is not in ${qml}")
        endif()
        if(name IN_LIST singletons)
            set_source_files_properties("${path}" PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
        endif()
        list(APPEND paths "${path}")
    endforeach()
    foreach(name ${lucide_files})
        set(path "${arg_LUCIDE_QT_DIR}/qml/Lucide/${name}")
        if(name IN_LIST lucide_singletons)
            set_source_files_properties("${path}" PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
        endif()
        list(APPEND paths "${path}")
    endforeach()
    # Files from outside the consumer's tree keep their own names in the module.
    foreach(path ${paths})
        get_filename_component(name "${path}" NAME)
        set_source_files_properties("${path}" PROPERTIES QT_RESOURCE_ALIAS "${name}")
    endforeach()

    string(REPLACE "." "/" uri_path "${uri}")
    qt_add_qml_module(${target}
        URI ${uri}
        VERSION ${version}
        STATIC
        OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/qml/${uri_path}"
        QML_FILES ${paths}
        NO_LINT)
    foreach(qt ${qt_modules})
        target_link_libraries(${target} PRIVATE Qt6::${qt})
    endforeach()

    # Style modules (BlockworkStyle) are plain resource modules with their own qmldir.
    string(JSON style_count LENGTH "${json}" style_modules)
    if(style_count GREATER 0)
        math(EXPR last "${style_count} - 1")
        foreach(i RANGE ${last})
            string(JSON style MEMBER "${json}" style_modules ${i})
            _blockwork_json_list("${json}" style_files style_modules ${style})
            list(TRANSFORM style_files PREPEND "${qml}/${style}/")
            _blockwork_add_resource(${target} "${target}_${style}" "/qt/qml/${style}" ${style_files})
        endforeach()
    endif()
    # Images the editor addresses by resource path (":/icons/blockwork.png").
    string(JSON res_count LENGTH "${json}" editor_resources)
    if(res_count GREATER 0)
        math(EXPR last "${res_count} - 1")
        foreach(i RANGE ${last})
            string(JSON prefix MEMBER "${json}" editor_resources ${i})
            _blockwork_json_list("${json}" res_files editor_resources ${prefix})
            list(TRANSFORM res_files PREPEND "${qml}/")
            set(res_paths)
            foreach(path ${res_files})
                cmake_path(NORMAL_PATH path)
                list(APPEND res_paths "${path}")
            endforeach()
            string(MAKE_C_IDENTIFIER "${prefix}" res_name)
            _blockwork_add_resource(${target} "${target}${res_name}" "${prefix}" ${res_paths})
        endforeach()
    endif()
    set(BLOCKWORK_QML_URI "${uri}" PARENT_SCOPE)
endfunction()
