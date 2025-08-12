# `rs_dfu`

A high-performance DFU (Device Firmware Update) implementation for flashing
STM32 devices in DFU mode. Built in Rust with C++ interoperability, rs_dfu is
specifically designed for flashing EdgeTX radios and supports both traditional
raw binaries and modern UF2 firmware files.

## Features

- **Multiple Firmware Formats**: 
  - Raw binary files
  - UF2 (USB Flashing Format) firmware files
- **EdgeTX Radio Support**: Optimized for EdgeTX radio hardware, including new platforms with external FLASH
- **Dual Interface**: 
  - Command-line tool (`rdfu`) for direct device flashing
  - C++ library for embedding in custom applications
- **Cross-Platform**: Support for Windows, macOS, and Linux

## Supported Platforms

| Platform | Architecture          | CLI Tool | Library |
|----------|-----------------------|----------|---------|
| Windows  | x86_64                |    ✅    |   ✅    |
| Windows  | ARM64                 |    ✅    |   ✅    |
| macOS    | x86_64 (Intel)        |    ✅    |   ✅    |
| macOS    | ARM64 (Apple Silicon) |    ✅    |   ✅    |
| Linux    | x86_64                |    ✅    |   ✅    |
| Linux    | ARM64                 |    ✅    |   ✅    |

## Installation

### Pre-built Releases

Download the latest release for your platform from the [releases page](https://github.com/EdgeTX/rs-dfu/releases):

- **Command-line tool**: `rdfu-{os-arch}`
- **C++ library**: `rs_dfu-{target-triple}.tar.gz` containing static library and header

### Building from Source

```bash
git clone https://github.com/EdgeTX/rs-dfu.git
cd rs-dfu
cargo build --all --release
```

## Command Line Usage

### Basic Operations

List all DFU devices:
```bash
rdfu list
```

Write firmware to device (auto-detects UF2 vs raw binary):
```bash
rdfu write firmware.uf2
rdfu write firmware.bin
```

### Device Selection

Filter devices by vendor/product ID:
```bash
# List only STM32 DFU devices
rdfu list --vendor 0483 --product df11

# Write to specific device
rdfu write --vendor 0483 --product df11 firmware.bin
```

### Advanced Options

Write raw binary to custom address:
```bash
rdfu write --start-address 0x08000000 firmware.bin
```

Reboot EdgeTX radio into DFU bootloader:
```bash
# Reboot with tag address
rdfu reboot 0x08000000

# Reboot specific device 
rdfu reboot --vendor 0483 --product df11 0x08000000
```

Inspect UF2 file contents:
```bash
rdfu uf2 firmware.uf2
```

## C++ Library Usage

### CMake Integration

Extract the library archive and use CMake's `find_package`:

```cmake
# Extract rs_dfu-{target}.tar.gz to your project
find_package(rs_dfu REQUIRED)

target_link_libraries(your_app rs_dfu::rs_dfu)
```

### Code Example

```cpp
#include "rs_dfu.h"
#include <stdio.h>

using ::rust::Vec;

void print_devices(const Vec<DfuDevice> &devices)
{
    for (const auto &device : devices) {
        auto dev_info = device.device_info();
        printf("%04x:%04x: %s (0x%08x)\n",
               dev_info.vendor_id,
               dev_info.product_id,
               dev_info.product_string.c_str(),
               device.default_start_address());
    }
}

int main() {
    try {
        // Find DFU devices
        auto device_filter = DfuDeviceFilter::empty_filter();
        auto devices = device_filter->find_devices();
        
        if (devices.empty()) {
            printf("No DFU devices found\n");
            return 1;
        }

        print_devices(devices);
        
    } catch (const std::exception& e) {
        printf("Error: %s\n", e.what());
        return 1;
    }
    
    return 0;
}
```

For a complete example including UF2 handling, error management, and progress
reporting, see [`examples/cpp/main.cpp`](examples/cpp/main.cpp).

## Development

### Building the Library

The project uses `cxx` for C++ interoperability:

```bash
# Build Rust library
cargo build --all --release

# Generate distribution package
./package.sh
```

This creates a distribution archive containing:
- Static and dynamic libraries
- C++ header files
- CMake configuration files

### Running Tests

```bash
cargo test
```

## License

This project is licensed under the [MIT License](LICENSE) - see the LICENSE file for details.

## Acknowledgments

- Built for the EdgeTX community
- Inspired by the DFU specification and existing DFU tools
- Uses the USB DFU protocol as defined by the USB Implementers Forum

## Support

For issues, questions, or contributions:
- Open an issue on [GitHub](https://github.com/EdgeTX/rs-dfu/issues)
- Check the EdgeTX community forums for radio-specific questions
