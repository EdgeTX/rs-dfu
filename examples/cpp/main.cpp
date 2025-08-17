#include "lib.rs.h"
#include <fmt/base.h>
#include <fmt/color.h>

#include <algorithm>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <vector>

using namespace std::chrono_literals;

using SliceU8 = rust::Slice<const uint8_t>;
using ::rust::Vec;

void print_devices(const Vec<DfuDevice> &devices) {
  for (const auto &device : devices) {
    auto dev_info = device.device_info();
    fmt::print("{:#06x}:{:#06x}: {} ({:#010x})\n", dev_info.vendor_id,
               dev_info.product_id, dev_info.product_string.c_str(),
               device.default_start_address());

    for (const auto &interface : device.interfaces()) {
      fmt::print("  {}:{}: {}\n", interface.interface(),
                 interface.alt_setting(), interface.name().c_str());
      for (const auto &segment : interface.segments()) {
        fmt::print("    {:#010x} -> {:#010x}\n", segment.start_addr,
                   segment.end_addr);
      }
    }
  }
}

void update_erase_status(size_t page, size_t pages) {
  fmt::print("\r  Erasing page {:2} of {:2}{}", page, pages,
             page == pages ? "\n" : "");
  fflush(stdout);
}

void update_download_status(size_t bytes, size_t total) {
  int percent = (100 * bytes) / total;
  fmt::print("\r  Flashing {:3}%{}", percent, percent == 100 ? "\n" : "");
  fflush(stdout);
}

int write_region(const DfuDevice &device, uint32_t addr, const SliceU8 &data) {
  auto start_address = addr;
  auto end_address = start_address + data.size() - 1;

  auto ctx = device.start_download(start_address, end_address);
  auto erase_pages = ctx->get_erase_pages();
  auto pages = erase_pages.size();

  for (size_t i = 0; i < erase_pages.size(); i++) {
    update_erase_status(i + 1, pages);
    ctx->page_erase(erase_pages[i]);
  }

  size_t bytes_downloaded = 0;
  auto data_ptr = data.data();
  auto xfer_size = ctx->get_transfer_size();

  while (bytes_downloaded < data.size()) {
    auto single_xfer_size =
        std::min(uint32_t(xfer_size), uint32_t(data.size() - bytes_downloaded));
    bytes_downloaded += single_xfer_size;
    update_download_status(bytes_downloaded, data.size());
    ctx->download(addr, SliceU8(data_ptr, single_xfer_size));
    addr += single_xfer_size;
    data_ptr += single_xfer_size;
  }

  return 0;
}

template <typename Duration>
void reboot_and_rediscover(DfuDevice &device, uint32_t addr,
                           const SliceU8 &data, uint32_t reboot_addr,
                           Duration timeout) {
  fmt::println("Rebooting into DFU...");
  device.reboot(addr, data, reboot_addr);
  auto start = std::chrono::steady_clock::now();
  while (std::chrono::steady_clock::now() - start < timeout) {
    if (device.rediscover()) {
      return;
    }
  }
  throw std::runtime_error("timeout while reconnection to device");
}

int main(int argc, char *argv[]) {
  try {
    auto device_filter = DfuDeviceFilter::empty_filter();
    auto devices = device_filter->find_devices();

    if (devices.empty()) {
      fmt::print("No DFU device\n");
      return -1;
    }

    if (argc < 2) {
      print_devices(devices);
      return 0;
    }

    std::string filename{argv[1]};
    auto file_size = std::filesystem::file_size(filename);

    std::vector<uint8_t> buffer(file_size);
    std::ifstream f(filename, std::ios::binary);
    f.read(reinterpret_cast<char *>(buffer.data()), buffer.size());

    fmt::println("Resetting state...");
    auto &device = devices[0];
    device.reset_state();

    SliceU8 buffer_slice(buffer.data(), buffer.size());
    if (!is_uf2_payload(buffer_slice)) {
      auto addr = device.default_start_address();
      return write_region(device, addr, buffer_slice);
    }

    auto range_it = UF2RangeIterator::from_slice(buffer_slice);
    auto addr_range = UF2AddressRange::new_empty();

    while (range_it->next(*addr_range)) {
      auto addr = addr_range->start_address();
      const auto &payload = addr_range->payload();

      uint32_t reboot_address = 0;
      if (addr_range->reboot_address(reboot_address)) {
        reboot_and_rediscover(device, addr, payload, reboot_address, 30s);
      } else {
        write_region(device, addr, payload);
      }
    }
    device.leave();

    return 0;

  } catch (const std::exception &err) {
    fmt::println("Error: {}", err.what());
    return 1;
  }
}
