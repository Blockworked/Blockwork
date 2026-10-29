# CMake entry point for the com.blockworked.Blockstitch QML module. It reads
# qml/module.json, the same manifest build.rs registers from, so a CMake build
# gets exactly the files, singletons and Qt modules the Cargo crate does.
#
#   include(<blockstitch>/cmake/BlockstitchQml.cmake)
#   blockstitch_add_qml_module(blockstitch)    # STATIC; link blockstitchplugin
#
# Call it after qt_standard_project_setup(). Needs CMake 3.21 and Qt 6.10.
cmake_minimum_required(VERSION 3.21)

set(BLOCKSTITCH_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." CACHE INTERNAL "")
cmake_path(NORMAL_PATH BLOCKSTITCH_ROOT)

# Reads a JSON list at `key` of `json` into `out`.
function(blockstitch_json_list json key out)
    string(JSON count LENGTH "${json}" ${key})
    set(items)
    if(count GREATER 0)
        math(EXPR last "${count} - 1")
        foreach(i RANGE ${last})
            string(JSON item GET "${json}" ${key} ${i})
            list(APPEND items "${item}")
        endforeach()
    endif()
    set(${out} ${items} PARENT_SCOPE)
endfunction()

# Fails when `dir` holds .qml files the manifest doesn't list, or the reverse.
function(blockstitch_check_listing manifest dir files)
    file(GLOB on_disk RELATIVE "${dir}" "${dir}/*.qml")
    set(unlisted ${on_disk})
    list(REMOVE_ITEM unlisted ${files})
    set(missing ${files})
    list(REMOVE_ITEM missing ${on_disk})
    if(unlisted OR missing)
        message(FATAL_ERROR "${manifest} is out of step with ${dir}: not listed [${unlisted}], missing [${missing}]")
    endif()
endfunction()

function(blockstitch_add_qml_module target)
    set(manifest "${BLOCKSTITCH_ROOT}/qml/module.json")
    file(READ "${manifest}" json)
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${manifest}")
    string(JSON uri GET "${json}" uri)
    string(JSON version GET "${json}" version)
    string(JSON subdir GET "${json}" dir)
    blockstitch_json_list("${json}" files files)
    blockstitch_json_list("${json}" singletons singletons)
    blockstitch_json_list("${json}" qt_modules qt_modules)
    set(dir "${BLOCKSTITCH_ROOT}/qml/${subdir}")
    blockstitch_check_listing("${manifest}" "${dir}" "${files}")

    find_package(Qt6 6.10 REQUIRED COMPONENTS ${qt_modules})
    set(paths)
    foreach(name ${files})
        set(path "${dir}/${name}")
        # Files from outside the consumer's tree keep their own names in the module.
        set_source_files_properties("${path}" PROPERTIES QT_RESOURCE_ALIAS "${name}")
        if(name IN_LIST singletons)
            set_source_files_properties("${path}" PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
        endif()
        list(APPEND paths "${path}")
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
    set(BLOCKSTITCH_QML_URI "${uri}" PARENT_SCOPE)
endfunction()
