cmake_minimum_required(VERSION 3.20)
if(POLICY CMP0177)
    cmake_policy(SET CMP0177 NEW)
endif()
project(coveui-third-party LANGUAGES C CXX)

include(${CMAKE_SOURCE_DIR}/../cmake/NativePlatform.cmake)
include(${CMAKE_SOURCE_DIR}/../cmake/ArtifactBundleManifest.cmake)

message(STATUS "CoveUI third-party target triple: ${COVE_TRIPLE} (SPM: ${COVE_SPM_TRIPLE})")

set(COVE_VENDOR_DIR ${CMAKE_SOURCE_DIR}/../vendor CACHE PATH "Text artifact output directory")

set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static libraries" FORCE)
set(CMAKE_POSITION_INDEPENDENT_CODE ON CACHE BOOL "" FORCE)

# Helper: emit info.json for a single-variant staticLibrary artifact bundle
function(write_artifactbundle_info BUNDLE_DIR ARTIFACT_NAME LIB_REL_PATH ARTIFACT_VERSION)
    install(CODE "
        file(WRITE ${BUNDLE_DIR}/info.json
\"{
  \\\"schemaVersion\\\": \\\"1.0\\\",
  \\\"artifacts\\\": {
    \\\"${ARTIFACT_NAME}\\\": {
      \\\"type\\\": \\\"staticLibrary\\\",
      \\\"version\\\": \\\"${ARTIFACT_VERSION}\\\",
      \\\"variants\\\": [
        {
          \\\"path\\\": \\\"${LIB_REL_PATH}\\\",
          \\\"supportedTriples\\\": [\\\"${COVE_SPM_TRIPLE}\\\"],
          \\\"staticLibraryMetadata\\\": {
            \\\"headerPaths\\\": [\\\"${COVE_TRIPLE}/include\\\"]
          }
        }
      ]
    }
  }
}
\")
    ")
endfunction()

include(${CMAKE_SOURCE_DIR}/cmake/BuildFreeType.cmake)
include(${CMAKE_SOURCE_DIR}/cmake/BuildHarfBuzz.cmake)
# CoveUITerminal is desktop-only (pseudo terminals); mobile staging reuses
# this graph without needing Zig.
if(NOT CMAKE_SYSTEM_NAME STREQUAL "iOS" AND NOT ANDROID)
    include(${CMAKE_SOURCE_DIR}/cmake/BuildGhosttyVt.cmake)
endif()
