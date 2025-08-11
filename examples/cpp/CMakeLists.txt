cmake_minimum_required(VERSION 3.22)
project(cpp_example)

include(FetchContent)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS true)
set(CMAKE_OSX_DEPLOYMENT_TARGET 11.0)

set(CARGO_MANIFEST ${CMAKE_SOURCE_DIR}/../../Cargo.toml)
set(CARGO_TARGET_DIR ${CMAKE_SOURCE_DIR}/../../target)
set(CARGO_CXX_TARGET_DIR ${CARGO_TARGET_DIR}/cxxbridge/rs-dfu)

set(RS_DFU_SOURCE_FILE ${CMAKE_SOURCE_DIR}/../../src/lib.rs)
set(RS_DFU_BRIDGE_H ${CARGO_CXX_TARGET_DIR}/src/lib.rs.h)
set(RS_DFU_LIB_NAME
  ${CMAKE_STATIC_LIBRARY_PREFIX}rs_dfu${CMAKE_STATIC_LIBRARY_SUFFIX}
)
set(RS_DFU_LIB ${CARGO_TARGET_DIR}/debug/${RS_DFU_LIB_NAME})

set(CARGO_OUTPUT
  ${RS_DFU_BRIDGE_CPP}
  ${RS_DFU_BRIDGE_H}
  ${RS_DFU_LIB}
)

# {fmt} (see https://fmt.dev/)
FetchContent_Declare(
  fmt
  GIT_REPOSITORY https://github.com/fmtlib/fmt
  GIT_TAG        0c9fce2ffefecfdce794e1859584e25877b7b592) # 11.0.2
FetchContent_MakeAvailable(fmt)

# Add a custom command that builds the rust crate and client library
add_custom_command(
  OUTPUT ${CARGO_OUTPUT}
  COMMAND cargo build --manifest-path ${CARGO_MANIFEST}
  DEPENDS ${RS_DFU_SOURCE_FILE}
  USES_TERMINAL
  COMMENT "Running cargo..."
)
      
# Example program
add_executable(cpp-example
  ${RS_DFU_BRIDGE_H}
  main.cpp
)

target_include_directories(cpp-example PRIVATE
  ${CARGO_CXX_TARGET_DIR}/src
)

target_link_libraries(cpp-example fmt::fmt)
target_link_libraries(cpp-example ${RS_DFU_LIB})

if(APPLE)
  target_link_libraries(cpp-example
    "-framework IOKit"
    "-framework CoreFoundation"
    "-framework CoreServices"
  )
endif()

