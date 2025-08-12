# rs_dfu CMake Configuration File

include(CMakeFindDependencyMacro)

# Platform detection
if(WIN32)
    set(_rs_dfu_lib_prefix "")
    set(_rs_dfu_shared_ext "dll")
    set(_rs_dfu_static_ext "lib")
elseif(APPLE)
    set(_rs_dfu_lib_prefix "lib")
    set(_rs_dfu_shared_ext "dylib")
    set(_rs_dfu_static_ext "a")
else()
    set(_rs_dfu_lib_prefix "lib")
    set(_rs_dfu_shared_ext "so")
    set(_rs_dfu_static_ext "a")
endif()

set(RS_DFU_DIR "${CMAKE_CURRENT_LIST_DIR}/..")
set(RS_DFU_INCLUDE_DIR "${RS_DFU_DIR}/include")
set(RS_DFU_LIB_DIR "${RS_DFU_DIR}/lib")

set(_rs_dfu_static_lib "${RS_DFU_LIB_DIR}/${_rs_dfu_lib_prefix}rs_dfu.${_rs_dfu_static_ext}")

# Create imported target
if(EXISTS "${_rs_dfu_static_lib}" AND NOT TARGET rs_dfu::static)
    add_library(rs_dfu::static STATIC IMPORTED)
    set_target_properties(rs_dfu::static PROPERTIES
        IMPORTED_LOCATION "${_rs_dfu_static_lib}"
        INTERFACE_INCLUDE_DIRECTORIES "${RS_DFU_INCLUDE_DIR}"
    )
endif()

# Create default target
if(NOT TARGET rs_dfu::rs_dfu)
    if(TARGET rs_dfu::static)
        add_library(rs_dfu::rs_dfu ALIAS rs_dfu::static)
    else()
        message(FATAL_ERROR "No rs_dfu libraries found!")
    endif()
endif()

set(RS_DFU_FOUND TRUE)
set(RS_DFU_INCLUDE_DIRS "${RS_DFU_INCLUDE_DIR}")
set(RS_DFU_LIBRARIES rs_dfu::rs_dfu)
