cmake_minimum_required(VERSION 3.14)

project(c_examples DESCRIPTION "libmicrovmi C examples" LANGUAGES C)

set(CMAKE_C_STANDARD 99)
if (WIN32)
    set(MICROVMI_FILENAME "microvmi.dll")
else ()
    set(MICROVMI_FILENAME "libmicrovmi.so")
endif ()

if (WIN32)
    # create symlink microvmi.lib -> microvmi.dll.lib in target/debug
    # because link.exe will try to look for microvmi.lib, and cargo uses another
    # naming it seems
    file(CREATE_LINK
        # original
        microvmi.dll.lib
        # link name
        "${CMAKE_CURRENT_SOURCE_DIR}/../target/debug/microvmi.lib"
        RESULT LINK_RES
        SYMBOLIC
    )
endif ()


# generate libmicrovmi.h
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/libmicrovmi.h"
    COMMAND cbindgen --config cbindgen.toml --crate microvmi --output "${CMAKE_CURRENT_BINARY_DIR}/libmicrovmi.h"
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../target/debug/${MICROVMI_FILENAME}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/.."
    COMMENT "Generating libmicrovmi.h with cbindgen")

# add local libmicrovmi.so library path
link_directories(
    # microvmi.dll and mirovmi.dll.lib are located in target/<debug|release>
    "${CMAKE_CURRENT_SOURCE_DIR}/../target/debug"
)
# add headers path
include_directories("${CMAKE_CURRENT_BINARY_DIR}")

add_executable(mem-dump mem-dump.c "${CMAKE_CURRENT_BINARY_DIR}/libmicrovmi.h")
target_link_libraries(mem-dump PRIVATE microvmi)

add_executable(regs-dump regs-dump.c "${CMAKE_CURRENT_BINARY_DIR}/libmicrovmi.h")
target_link_libraries(regs-dump PRIVATE microvmi)

if (NOT WIN32)
    # TODO: libmicrovmi.h DriverType enum conflict with same name in winnt.h
    # disable pause example for windows
    add_executable(pause pause.c "${CMAKE_CURRENT_BINARY_DIR}/libmicrovmi.h")
    target_link_libraries(pause PRIVATE microvmi)
endif ()
