use std::mem;
use libusb1_sys::constants as libusb;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct InterfaceDesc {
    length: u8,
    desc_type: u8,
    interface_num: u8,
    alt_setting: u8,
    num_endpoints: u8,
    class: u8,
    subclass: u8,
    proto: u8,
    interface: u8,
}

const USB_DT_INTERFACE_SIZE: usize = 9;
const ADB_CLASS: u8 = 0xff;
const ADB_SUBCLASS: u8 = 0x42;
const ADB_PROTOCOL: u8 = 0x1;

const ADB_INTERFACE: InterfaceDesc = InterfaceDesc {
    length: USB_DT_INTERFACE_SIZE as u8,
    desc_type: libusb::LIBUSB_DT_INTERFACE,
    interface_num: 0,
    alt_setting: 0,
    num_endpoints: 2,
    class: ADB_CLASS,
    subclass: ADB_SUBCLASS,
    proto: ADB_PROTOCOL,
    interface: 1,
};

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct EndpointDescNoAudio {
    length: u8,
    desc_type: u8,
    addr: u8,
    attr: u8,
    max_packet_size: u16,
    interval: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct OsHeader {
    iface: u8,
    len: u32,
    bcd_ver: u16,
    idx: u16,
    count: u8,
    reserved: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct ExtCompatDesc {
    first_iface_num: u8,
    res1: u8,
    compat_id: [u8; 8],
    sub_compat_id: [u8; 8],
    res_2: [u8; 6],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct FuncDesc {
    interface: InterfaceDesc,
    source: EndpointDescNoAudio,
    sink: EndpointDescNoAudio,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct OsPropValues {
    len: u32,
    data_type: u32,
    name_len: u16,
    name: [u8; DEV_IFACE_GUID.len()],
    prop_len: u32,
    prop: [u8; GUID.len()],
}

const DEV_IFACE_GUID: &'static [u8; 20] = b"DeviceInterfaceGUID\0";
const GUID: &'static [u8; 39] = b"{F72FE0D4-CBCB-407D-8814-9ED673D0DD6B}\0";

const OS_PROP_VALUES: OsPropValues = OsPropValues {
    len: mem::size_of::<OsPropValues>() as u32,
    data_type: 1,
    name_len: DEV_IFACE_GUID.len() as u16,
    name: *DEV_IFACE_GUID,
    prop_len: GUID.len() as u32,
    prop: *GUID,
};

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct SsEpCompDesc {
    length: u8,
    desc_type: u8,
    max_burst: u8,
    attr: u8,
    bytes_per_interval: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct SsFuncDesc {
    interface: InterfaceDesc,
    source: EndpointDescNoAudio,
    source_comp: SsEpCompDesc,
    sink: EndpointDescNoAudio,
    sink_comp: SsEpCompDesc,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct DescV2 {
    magic: u32,
    length: u32,
    flags: u32,
    fs_count: u32,
    hs_count: u32,
    ss_count: u32,
    os_count: u32,
    fs_descs: FuncDesc,
    hs_descs: FuncDesc,
    ss_descs: SsFuncDesc,
    os_header: OsHeader,
    os_desc: ExtCompatDesc,
    os_prop_header: OsHeader,
    os_prop_values: OsPropValues,
}

impl DescV2 {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const DescV2 as *const u8,
                mem::size_of::<DescV2>(),
            )
        }
    }
}

const FUNCTIONFS_STRINGS_MAGIC: u32 = 2_u32.to_le();
const FUNCTIONFS_DESCRIPTORS_MAGIC_V2: u32 = 3_u32.to_le();

const FFS_HAS_FS_DESC: u32 = 1_u32.to_le();
const FFS_HAS_HS_DESC: u32 = 2_u32.to_le();
const FFS_HAS_SS_DESC: u32 = 4_u32.to_le();
const FFS_HAS_MS_OS_DESC: u32 = 8_u32.to_le();

const MAX_PACKET_SIZE_FS: u16 = 64_u16.to_le();
const MAX_PACKET_SIZE_HS: u16 = 512_u16.to_le();
const MAX_PACKET_SIZE_SS: u16 = 1024_u16.to_le();

const USB_DIR_OUT: u8 = 0;
const USB_DIR_IN: u8 = 0x80;

const USB_ENDPOINT_XFER_BULK: u8 = 2;

pub const ADB_DESCRIPTOR_V2: DescV2 = DescV2 {
    magic: FUNCTIONFS_DESCRIPTORS_MAGIC_V2,
    length: (mem::size_of::<DescV2>() as u32).to_le(),
    flags: FFS_HAS_FS_DESC | FFS_HAS_HS_DESC |
           FFS_HAS_SS_DESC | FFS_HAS_MS_OS_DESC,
    fs_count: 3_u32.to_le(),
    hs_count: 3_u32.to_le(),
    ss_count: 5_u32.to_le(),
    os_count: 2_u32.to_le(),
    fs_descs: ADB_FS_DESC,
    hs_descs: ADB_HS_DESC,
    ss_descs: ADB_SS_DESC,
    os_header: OS_DESC_HEADER,
    os_desc: OS_DESC_COMPAT,
    os_prop_header: OS_PROP_HEADER,
    os_prop_values: OS_PROP_VALUES,
};

const ADB_FS_DESC: FuncDesc = FuncDesc {
    interface: ADB_INTERFACE,
    source: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 1 | USB_DIR_OUT,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_FS,
        interval: 0,
    },
    sink: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 2 | USB_DIR_IN,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_FS,
        interval: 0,
    },
};

const ADB_HS_DESC: FuncDesc = FuncDesc {
    interface: ADB_INTERFACE,
    source: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 1 | USB_DIR_OUT,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_HS,
        interval: 0,
    },
    sink: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 2 | USB_DIR_IN,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_HS,
        interval: 0,
    },
};

const ADB_SS_DESC: SsFuncDesc = SsFuncDesc {
    interface: ADB_INTERFACE,
    source: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 1 | USB_DIR_OUT,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_SS,
        interval: 0,
    },
    source_comp: SsEpCompDesc {
        length: mem::size_of::<SsEpCompDesc>() as u8,
        desc_type: libusb::LIBUSB_DT_SS_ENDPOINT_COMPANION,
        max_burst: 4,
        attr: 0,
        bytes_per_interval: 0,
    },
    sink: EndpointDescNoAudio {
        length: mem::size_of::<EndpointDescNoAudio>() as u8,
        desc_type: libusb::LIBUSB_DT_ENDPOINT,
        addr: 2 | USB_DIR_IN,
        attr: USB_ENDPOINT_XFER_BULK,
        max_packet_size: MAX_PACKET_SIZE_SS,
        interval: 0,
    },
    sink_comp: SsEpCompDesc {
        length: mem::size_of::<SsEpCompDesc>() as u8,
        desc_type: libusb::LIBUSB_DT_SS_ENDPOINT_COMPANION,
        max_burst: 4,
        attr: 0,
        bytes_per_interval: 0,
    },
};

const OS_DESC_HEADER: OsHeader = OsHeader {
    iface: 0,
    len: (mem::size_of::<OsHeader>() + mem::size_of::<ExtCompatDesc>()) as u32,
    bcd_ver: 1,
    idx: 4,
    count: 1,
    reserved: 0,
};

const OS_DESC_COMPAT: ExtCompatDesc = ExtCompatDesc {
    first_iface_num: 0,
    res1: 1,
    compat_id: *b"WINUSB\0\0",
    sub_compat_id: [0_u8; 8],
    res_2: [0; 6],
};

const OS_PROP_HEADER: OsHeader = OsHeader {
    iface: 0,
    len: (mem::size_of::<OsHeader>() + mem::size_of::<OsPropValues>()) as u32,
    bcd_ver: 1,
    idx: 5,
    count: 1,
    reserved: 0,
};

const IFACE_STRING: &'static [u8; 14] = b"ADB Interface\0";

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FfsStringData {
    magic: u32,
    length: u32,
    str_count: u32,
    lang_count: u32,
    code: u16,
    str1: [u8; IFACE_STRING.len()],
}

impl FfsStringData {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FfsStringData as *const u8,
                mem::size_of::<FfsStringData>(),
            )
        }
    }
}

pub const ADB_STRINGS: FfsStringData = FfsStringData {
    magic: FUNCTIONFS_STRINGS_MAGIC,
    length: (mem::size_of::<FfsStringData>() as u32).to_le(),
    str_count: 1,
    lang_count: 1,

    code: 0x409_u16.to_le(),
    str1: *IFACE_STRING,
};
