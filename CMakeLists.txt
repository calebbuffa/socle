cmake_minimum_required(VERSION 3.15)
project(socle VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)

option(SOCLE_CARGO_BUILD
    "Run `cargo build` automatically during CMake build" ON)


if(CMAKE_BUILD_TYPE STREQUAL "Release" OR CMAKE_BUILD_TYPE STREQUAL "RelWithDebInfo")
    set(CARGO_PROFILE "release")
    set(CARGO_PROFILE_FLAG "--release")
    set(CARGO_OUTPUT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/target/release")
else()
    set(CARGO_PROFILE "debug")
    set(CARGO_PROFILE_FLAG "")
    set(CARGO_OUTPUT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/target/debug")
endif()

set(ORKESTER_LIB_NAME
    "${CMAKE_STATIC_LIBRARY_PREFIX}orkester_ffi${CMAKE_STATIC_LIBRARY_SUFFIX}")

if(SOCLE_CARGO_BUILD)
    find_program(CARGO_EXECUTABLE cargo REQUIRED)

    add_custom_target(cargo_build ALL
        COMMAND ${CARGO_EXECUTABLE} build -p orkester-ffi ${CARGO_PROFILE_FLAG}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
        COMMENT "Building orkester-ffi Rust static library (${CARGO_PROFILE})"
    )
endif()

# ── platform link dependencies (Rust std runtime) ────────────────────────────

if(WIN32)
    set(RUST_PLATFORM_LIBS ws2_32 userenv bcrypt ntdll advapi32)
    if(MSVC OR CMAKE_CXX_SIMULATE_ID STREQUAL "MSVC")
        list(APPEND RUST_PLATFORM_LIBS synchronization)
    endif()
elseif(APPLE)
    set(RUST_PLATFORM_LIBS "-framework Security" "-framework CoreFoundation" dl pthread)
else()
    set(RUST_PLATFORM_LIBS dl pthread m)
endif()

# ── imported targets (for use when socle is added via add_subdirectory) ──────

add_library(orkester STATIC IMPORTED GLOBAL)
set_target_properties(orkester PROPERTIES
    IMPORTED_LOCATION "${CARGO_OUTPUT_DIR}/${ORKESTER_LIB_NAME}"
)
target_include_directories(orkester INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
)
target_link_libraries(orkester INTERFACE ${RUST_PLATFORM_LIBS})

if(SOCLE_CARGO_BUILD)
    add_dependencies(orkester cargo_build)
endif()

# Alias that matches the installed package target name
add_library(orkester::ffi ALIAS orkester)

# ── install rules ────────────────────────────────────────────────────────────

# Install static library
install(FILES
    "${CARGO_OUTPUT_DIR}/${ORKESTER_LIB_NAME}"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}
)

# Install header
install(FILES
    "${CMAKE_CURRENT_SOURCE_DIR}/include/orkester.h"
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
)

# Install CMake package config files
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/orkesterConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/orkesterConfig.cmake"
    COPYONLY
)

write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/orkesterConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY AnyNewerVersion
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/orkesterConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/orkesterConfigVersion.cmake"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/orkester
)
