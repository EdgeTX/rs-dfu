//! Main crate

#[cxx::bridge]
mod ffi {

    struct DeviceInfo {
        vendor_id: u16,
        product_id: u16,
        product_string: String,
    }

    struct MemorySegment {
        start_addr: u32,
        end_addr: u32,
        page_size: u32,
        readable: bool,
        writable: bool,
        erasable: bool,
    }

    extern "Rust" {
        type DfuDeviceFilter;

        #[Self = "DfuDeviceFilter"]
        fn empty_filter() -> Box<DfuDeviceFilter>;

        fn with_vendor_id(&mut self, vid: u16);
        fn with_product_id(&mut self, pid: u16);
        fn find_devices(&self) -> Result<Vec<DfuDevice>>;
    }

    extern "Rust" {
        type DfuDevice;

        fn device_info(&self) -> DeviceInfo;
        fn interfaces(&self) -> Vec<DfuInterface>;
        fn reset_state(&self) -> Result<()>;
        fn default_start_address(&self) -> u32;
        fn start_upload(
            &self,
            start_address: u32,
            length: u32,
        ) -> Result<Box<DfuUpload>>;
        fn start_download(
            &self,
            start_address: u32,
            end_address: u32,
        ) -> Result<Box<DfuDownload>>;
        fn reboot(
            &self,
            addr: u32,
            data: &[u8],
            reboot_addr: u32,
        ) -> Result<()>;
        fn rediscover(&mut self) -> Result<bool>;
        fn leave(&self) -> Result<()>;
    }

    extern "Rust" {
        type DfuInterface;

        fn name(&self) -> String;
        fn interface(&self) -> u8;
        fn alt_setting(&self) -> u8;
        fn segments(&self) -> Vec<MemorySegment>;
    }

    extern "Rust" {
        type DfuUpload;

        fn get_length(&self) -> u32;
        fn get_transfer_size(&self) -> u16;
        fn upload(&mut self, length: u16) -> Result<Vec<u8>>;
    }

    extern "Rust" {
        type DfuDownload;

        fn get_erase_pages(&self) -> Vec<u32>;
        fn get_transfer_size(&self) -> u16;
        fn page_erase(&self, addr: u32) -> Result<()>;
        fn download(&self, addr: u32, data: &[u8]) -> Result<()>;
    }

    extern "Rust" {
        type UF2RangeIterator<'a>;

        #[Self = "UF2RangeIterator"]
        unsafe fn from_slice<'a>(
            data: &'a [u8],
        ) -> Result<Box<UF2RangeIterator<'a>>>;

        fn next(&mut self, addr_range: &mut UF2AddressRange) -> bool;
    }

    extern "Rust" {
        type UF2AddressRange;

        #[Self = "UF2AddressRange"]
        fn new_empty() -> Box<UF2AddressRange>;

        fn start_address(self: &UF2AddressRange) -> u32;
        fn payload(self: &UF2AddressRange) -> &[u8];
        fn reboot_address(self: &UF2AddressRange, addr: &mut u32) -> bool;

        fn is_uf2_payload(data: &[u8]) -> bool;
    }
}

#[derive(Default)]
pub struct DfuDeviceFilter {
    vid: Option<u16>,
    pid: Option<u16>,
}

pub struct DfuDevice {
    inner: dfu::DfuDevice,
}

pub struct DfuInterface {
    inner: dfu::DfuInterface,
}

pub struct DfuUpload {
    connection: dfu::DfuConnection,
    length: u32,
    block_nr: u16,
}

pub struct DfuDownload {
    connection: dfu::DfuConnection,
    erase_pages: Vec<u32>,
}

impl DfuDeviceFilter {
    fn empty_filter() -> Box<DfuDeviceFilter> {
        Box::new(DfuDeviceFilter::default())
    }

    fn with_vendor_id(&mut self, vid: u16) {
        self.vid.replace(vid);
    }

    fn with_product_id(&mut self, pid: u16) {
        self.pid.replace(pid);
    }

    fn find_devices(&self) -> Result<Vec<DfuDevice>, dfu::DfuError> {
        dfu::find_dfu_devices(self.vid, self.pid)
            .map(|devices| devices.into_iter().map(DfuDevice::new).collect())
    }
}

impl DfuDevice {
    fn new(device: dfu::DfuDevice) -> Self {
        DfuDevice { inner: device }
    }

    fn device_info(&self) -> ffi::DeviceInfo {
        ffi::DeviceInfo {
            vendor_id: self.inner.vendor_id(),
            product_id: self.inner.product_id(),
            product_string: self.inner.product_string().unwrap_or("").into(),
        }
    }

    fn interfaces(&self) -> Vec<DfuInterface> {
        self.inner
            .interfaces()
            .iter()
            .map(|intf| DfuInterface::new(intf.to_owned()))
            .collect()
    }

    fn reset_state(&self) -> Result<(), dfu::DfuError> {
        let connection = self.inner.connect(0, 0)?;
        connection.reset_state()
    }

    fn default_start_address(&self) -> u32 {
        self.inner.get_default_start_address()
    }

    fn start_upload(
        &self,
        start_address: u32,
        length: u32,
    ) -> Result<Box<DfuUpload>, dfu::DfuError> {
        let end_address = if length > 0 {
            Some(start_address + length - 1)
        } else {
            None
        };
        let intf = self.inner.find_interface(start_address, end_address)?;
        let segments = intf.find_segments(start_address, end_address);
        if segments.is_empty() {
            return Err(dfu::DfuError::NoMemorySegments);
        }

        let end_address =
            end_address.unwrap_or(segments.last().unwrap().end_addr() - 1);
        let length = end_address - start_address + 1;
        let connection =
            self.inner.connect(intf.interface(), intf.alt_setting())?;

        Ok(Box::new(DfuUpload {
            connection,
            length,
            block_nr: 0,
        }))
    }

    fn start_download(
        &self,
        start_address: u32,
        end_address: u32,
    ) -> Result<Box<DfuDownload>, dfu::DfuError> {
        let intf = self
            .inner
            .find_interface(start_address, Some(end_address))?;
        let erase_pages = intf.get_erase_pages(start_address, end_address);
        let connection =
            self.inner.connect(intf.interface(), intf.alt_setting())?;
        Ok(Box::new(DfuDownload {
            connection,
            erase_pages,
        }))
    }

    fn reboot(
        &self,
        addr: u32,
        data: &[u8],
        reboot_addr: u32,
    ) -> Result<(), dfu::DfuError> {
        let connection = self.inner.connect(0, 0)?;
        connection.reboot(addr, data, reboot_addr)
    }

    fn rediscover(&mut self) -> Result<bool, dfu::DfuError> {
        let devices = dfu::find_dfu_devices(
            Some(self.inner.vendor_id()),
            Some(self.inner.product_id()),
        )?;
        Ok(if !devices.is_empty() {
            self.inner = devices.into_iter().next().unwrap();
            true
        } else {
            false
        })
    }

    fn leave(&self) -> Result<(), dfu::DfuError> {
        let connection = self.inner.connect(0, 0)?;
        connection.leave()
    }
}

impl DfuInterface {
    fn new(interface: dfu::DfuInterface) -> Self {
        DfuInterface { inner: interface }
    }

    fn name(&self) -> String {
        self.inner.layout().name.clone()
    }

    fn interface(&self) -> u8 {
        self.inner.interface()
    }

    fn alt_setting(&self) -> u8 {
        self.inner.alt_setting()
    }

    fn segments(&self) -> Vec<ffi::MemorySegment> {
        self.inner
            .layout()
            .segments
            .iter()
            .map(ffi::MemorySegment::from_dfu_segment)
            .collect()
    }
}

impl DfuUpload {
    fn get_transfer_size(&self) -> u16 {
        self.connection.transfer_size()
    }

    fn get_length(&self) -> u32 {
        self.length
    }

    fn upload(&mut self, length: u16) -> Result<Vec<u8>, dfu::DfuError> {
        let data = self.connection.upload(self.block_nr, length)?;
        self.block_nr += 1;
        Ok(data)
    }
}

impl DfuDownload {
    fn get_erase_pages(&self) -> Vec<u32> {
        self.erase_pages.clone()
    }

    fn get_transfer_size(self: &DfuDownload) -> u16 {
        self.connection.transfer_size()
    }

    fn page_erase(&self, addr: u32) -> Result<(), dfu::DfuError> {
        self.connection.dfuse_page_erase(addr)
    }

    fn download(&self, addr: u32, data: &[u8]) -> Result<(), dfu::DfuError> {
        self.connection.download(addr, data)
    }
}

impl ffi::MemorySegment {
    fn from_dfu_segment(segment: &dfu::DfuMemSegment) -> Self {
        ffi::MemorySegment {
            start_addr: segment.start_addr(),
            end_addr: segment.end_addr(),
            page_size: segment.page_size(),
            readable: segment.readable(),
            writable: segment.writable(),
            erasable: segment.erasable(),
        }
    }
}

pub struct UF2RangeIterator<'a> {
    inner: uf2::UF2RangeIterator<'a>,
}

#[derive(Default)]
pub struct UF2AddressRange {
    inner: uf2::UF2AddressRange,
}

impl<'a> UF2RangeIterator<'a> {
    fn from_slice(
        data: &'a [u8],
    ) -> Result<Box<UF2RangeIterator<'a>>, uf2::UF2DecodeError> {
        Ok(Box::new(UF2RangeIterator {
            inner: uf2::UF2RangeIterator::new(data)?,
        }))
    }

    fn next(&mut self, addr_range: &mut UF2AddressRange) -> bool {
        match self.inner.next() {
            Some(elmt) => {
                addr_range.inner = elmt;
                true
            }
            None => false,
        }
    }
}

impl UF2AddressRange {
    fn new_empty() -> Box<UF2AddressRange> {
        Box::new(UF2AddressRange::default())
    }

    fn start_address(self: &UF2AddressRange) -> u32 {
        self.inner.start_address
    }

    fn payload(self: &UF2AddressRange) -> &[u8] {
        &self.inner.payload
    }

    fn reboot_address(self: &UF2AddressRange, addr: &mut u32) -> bool {
        match &self.inner.reboot_address {
            Some(val) => {
                *addr = *val;
                true
            }
            None => false,
        }
    }
}

pub fn is_uf2_payload(data: &[u8]) -> bool {
    uf2::is_uf2_payload(data)
}
