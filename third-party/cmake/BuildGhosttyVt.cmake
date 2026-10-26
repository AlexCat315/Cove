# Builds libghostty-vt (Ghostty's zero-dependency VT emulator library) with
# Zig and stages it into vendor/GhosttyVt.artifactbundle/<triple>/.
#
# Ghostty's build is Zig-only; there is no CMake project to delegate to, so
# this drives `zig build` directly. Zig caches live in the build tree so the
# submodule checkout stays clean; fetched Zig packages use the shared global
# cache like any other Zig build.

include(ExternalProject)

set(GHOSTTY_ZIG_VERSION "0.16.0")
find_program(COVE_ZIG zig)
if(NOT COVE_ZIG)
    message(FATAL_ERROR
        "zig ${GHOSTTY_ZIG_VERSION} is required to build libghostty-vt for CoveUITerminal.\n"
        "Install it from https://ziglang.org/download/ or pass -DCOVE_ZIG=/path/to/zig.")
endif()
execute_process(COMMAND ${COVE_ZIG} version OUTPUT_VARIABLE _zig_version OUTPUT_STRIP_TRAILING_WHITESPACE)
if(NOT _zig_version STREQUAL GHOSTTY_ZIG_VERSION)
    message(FATAL_ERROR "libghostty-vt needs zig ${GHOSTTY_ZIG_VERSION}; found ${_zig_version} at ${COVE_ZIG}.\n"
        "Point -DCOVE_ZIG=/path/to/zig at a ${GHOSTTY_ZIG_VERSION} toolchain.")
endif()

# COVE_TRIPLE → Zig target. Deployment floors match NativePlatform.cmake.
set(_ghostty_targets
    "macos-arm64=aarch64-macos.26.0"
    "macos-x86_64=x86_64-macos.26.0"
    "linux-x86_64=x86_64-linux-gnu"
    "linux-aarch64=aarch64-linux-gnu"
    "windows-x86_64=x86_64-windows-msvc"
    "windows-arm64=aarch64-windows-msvc")
set(GHOSTTY_ZIG_TARGET "")
foreach(_entry IN LISTS _ghostty_targets)
    string(REPLACE "=" ";" _pair "${_entry}")
    list(GET _pair 0 _triple)
    if(_triple STREQUAL COVE_TRIPLE)
        list(GET _pair 1 GHOSTTY_ZIG_TARGET)
    endif()
endforeach()
if(GHOSTTY_ZIG_TARGET STREQUAL "")
    message(FATAL_ERROR "No Zig target mapping for ${COVE_TRIPLE}")
endif()

set(GHOSTTY_SOURCE_DIR ${CMAKE_SOURCE_DIR}/ghostty)
set(GHOSTTY_BUNDLE ${COVE_VENDOR_DIR}/GhosttyVt.artifactbundle)
set(GHOSTTY_VARIANT ${GHOSTTY_BUNDLE}/${COVE_TRIPLE})
set(GHOSTTY_INSTALL_PREFIX ${CMAKE_BINARY_DIR}/ghostty-install)
if(WIN32)
    set(GHOSTTY_LIB_FILENAME "ghostty-vt-static.lib")
else()
    set(GHOSTTY_LIB_FILENAME "libghostty-vt.a")
endif()

# Ghostty merges its Apple archives with `libtool -static`. Homebrew's GNU
# libtool shadows Apple's on many PATHs, so put the Xcode toolchain first.
set(_ghostty_env "")
if(APPLE)
    execute_process(COMMAND xcrun --find libtool OUTPUT_VARIABLE _apple_libtool OUTPUT_STRIP_TRAILING_WHITESPACE)
    get_filename_component(_apple_tool_dir "${_apple_libtool}" DIRECTORY)
    set(_ghostty_env ${CMAKE_COMMAND} -E env "PATH=${_apple_tool_dir}:$ENV{PATH}")
endif()

ExternalProject_Add(ghostty_vt_ep
    SOURCE_DIR ${GHOSTTY_SOURCE_DIR}
    PREFIX ${CMAKE_BINARY_DIR}/ghostty-ep
    INSTALL_DIR ${GHOSTTY_INSTALL_PREFIX}
    CONFIGURE_COMMAND ""
    BUILD_IN_SOURCE TRUE
    BUILD_COMMAND ${_ghostty_env} ${COVE_ZIG} build
        -Demit-lib-vt=true
        -Doptimize=ReleaseFast
        -Dtarget=${GHOSTTY_ZIG_TARGET}
        --prefix ${GHOSTTY_INSTALL_PREFIX}
        --cache-dir ${CMAKE_BINARY_DIR}/ghostty-zig-cache
    INSTALL_COMMAND ""
    BUILD_ALWAYS FALSE
)

add_custom_target(stage_ghostty_vt ALL
    DEPENDS ghostty_vt_ep
    COMMAND ${CMAKE_COMMAND} -E rm -rf ${GHOSTTY_VARIANT}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${GHOSTTY_VARIANT}/lib
    COMMAND ${CMAKE_COMMAND} -E make_directory ${GHOSTTY_VARIANT}/include
    COMMAND ${CMAKE_COMMAND} -E copy
        ${GHOSTTY_INSTALL_PREFIX}/lib/${GHOSTTY_LIB_FILENAME}
        ${GHOSTTY_VARIANT}/lib/${GHOSTTY_LIB_FILENAME}
    COMMAND ${CMAKE_COMMAND} -E copy_directory
        ${GHOSTTY_INSTALL_PREFIX}/include/ghostty ${GHOSTTY_VARIANT}/include/ghostty
    COMMAND ${CMAKE_COMMAND} -E copy
        ${CMAKE_SOURCE_DIR}/cmake/templates/GhosttyVt.h
        ${GHOSTTY_VARIANT}/include/GhosttyVt.h
    COMMAND ${CMAKE_COMMAND} -E copy
        ${CMAKE_SOURCE_DIR}/cmake/templates/GhosttyVt.modulemap
        ${GHOSTTY_VARIANT}/include/module.modulemap
    COMMENT "Staging libghostty-vt into ${GHOSTTY_VARIANT}"
)

cove_git_revision(GHOSTTY_SOURCE_REVISION "${GHOSTTY_SOURCE_DIR}")
string(SUBSTRING "${GHOSTTY_SOURCE_REVISION}" 0 7 _ghostty_short_revision)
set(GHOSTTY_VERSION "1.3.2-dev+${_ghostty_short_revision}")

write_artifactbundle_info(${GHOSTTY_BUNDLE} "GhosttyVt" "${COVE_TRIPLE}/lib/${GHOSTTY_LIB_FILENAME}" "${GHOSTTY_VERSION}")

cove_add_artifact_build_manifest(
    ARTIFACT_NAME "GhosttyVt"
    VERSION "${GHOSTTY_VERSION}"
    BUNDLE_DIR ${GHOSTTY_BUNDLE}
    LIB_REL_PATH "${COVE_TRIPLE}/lib/${GHOSTTY_LIB_FILENAME}"
    BUILD_SYSTEM "Zig ${GHOSTTY_ZIG_VERSION} (zig build -Demit-lib-vt=true)"
    SOURCE_KIND "git-submodule"
    SOURCE_URL "third-party/ghostty"
    SOURCE_REF "main"
    SOURCE_REVISION ${GHOSTTY_SOURCE_REVISION}
    C_COMPILER "${COVE_ZIG} (${GHOSTTY_ZIG_TARGET})"
    C_FLAGS_RELEASE "-Doptimize=ReleaseFast"
    NOTES "libghostty-vt static library, zero-dependency C ABI. Pre-1.0 and unstable: pin the submodule revision"
    CXX_COMPILER "none"
    CXX_FLAGS_RELEASE "none"
    DEPENDS stage_ghostty_vt
)
