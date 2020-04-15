use std::io::{self, Cursor};
use std::sync::Arc;
use bytes::*;
use tokio_util::codec::{Encoder, Decoder};

use usb_device::UsbDirection;
use usb_device::endpoint::EndpointAddress;

const VERSION: u32 = 0x01110000;
const OP_REQ_DEVLIST: u32 = VERSION | 0x8005;
const OP_REP_DEVLIST: u32 = VERSION | 0x0005;
const OP_REQ_IMPORT: u32 = VERSION | 0x8003;
const OP_REP_IMPORT: u32 = VERSION | 0x0003;
const OP_CMD_SUBMIT: u32 = 0x00000001;
const OP_RET_SUBMIT: u32 = 0x00000003;
const OP_CMD_UNLINK: u32 = 0x00000002;
const OP_RET_UNLINK: u32 = 0x00000004;

#[derive(Debug)]
pub enum Request {
    DevList,
    Import(String),
    Submit(SubmitRequest),
    Unlink(UnlinkRequest),
}

#[derive(Debug)]
pub enum Response {
    DevList(Vec<Arc<DeviceInterfaceInfo>>),
    Import(ImportResponse),
    Submit(SubmitResponse),
    Unlink(UnlinkResponse),
}

#[derive(Debug)]
pub struct DeviceInterfaceInfo {
    pub device: Arc<DeviceInfo>,
    pub interfaces: Vec<InterfaceInfo>,
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub path: String,
    pub busid: String,
    pub busnum: u32,
    pub devnum: u32,
    pub speed: u32,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub configuration_value: u8,
    pub num_configuration: u8,
    pub num_interfaces: u8,
}

#[derive(Debug)]
pub struct InterfaceInfo {
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
}

#[derive(Debug)]
pub struct ImportResponse {
    pub status: u32,
    pub device: Option<Arc<DeviceInfo>>,
}

#[derive(Debug)]
pub struct SubmitRequest {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress,
    pub transfer_flags: u32,
    pub transfer_buffer_length: u32,
    pub start_frame: u32,
    pub number_of_packets: u32,
    pub interval: u32,
    pub setup: Option<[u8; 8]>,
    pub data: BytesMut,
}

#[derive(Debug)]
pub struct SubmitResponse {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress,
    pub status: u32,
    pub actual_length: u32, // TODO: does this need to be filled in for OUT transactions?
    pub actual_start_frame: u32,
    pub number_of_packets: u32,
    pub error_count: u32,
    pub setup: Option<[u8; 8]>,
    pub data: BytesMut,
}

#[derive(Debug)]
pub struct UnlinkRequest {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress,
    pub unlink_seqnum: u32,
}

#[derive(Debug)]
pub struct UnlinkResponse {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress,
    pub status: u32,
    pub unlink_seqnum: u32,
}

fn invalid_data() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}

pub struct UsbIpCodec;

impl UsbIpCodec {
    const DEVICE_INFO_SIZE: usize = 256 + 32 + (3 * 4) + (3 * 2) + 6;
    const INTERFACE_INFO_SIZE: usize = 4;
    const URB_HEADER_SIZE: usize = 4 * 4;
    const PDU_LENGTH: usize = 48;

    pub fn new() -> Self {
        UsbIpCodec
    }

    fn decode_urb_header(c: &mut Cursor<BytesMut>) -> io::Result<(u32, u32, EndpointAddress)> {
        let seqnum = c.get_u32();
        let devid = c.get_u32();

        let direction = c.get_u32();
        if !(direction <= 1) {
            return Err(invalid_data());
        }

        let ep = c.get_u32();
        if !(ep <= 15) {
            return Err(invalid_data());
        }

        let ep = EndpointAddress::from_parts(
            ep as u8,
            if direction == 0 { UsbDirection::Out } else { UsbDirection::In });

        return Ok((seqnum, devid, ep));
    }

    fn encode_device_info(dev: &DeviceInfo, buf: &mut BytesMut) {
        let mut path = [0u8; 256];
        path[..dev.path.len()].copy_from_slice(dev.path.as_bytes());
        buf.extend_from_slice(&path[..]);

        let mut busid = [0u8; 32];
        busid[..dev.busid.len()].copy_from_slice(dev.busid.as_bytes());
        buf.extend_from_slice(&busid[..]);

        buf.put_u32(dev.busnum);
        buf.put_u32(dev.devnum);
        buf.put_u32(dev.speed);

        buf.put_u16(dev.id_vendor);
        buf.put_u16(dev.id_product);
        buf.put_u16(dev.bcd_device);

        buf.put_u8(dev.device_class);
        buf.put_u8(dev.device_subclass);
        buf.put_u8(dev.device_protocol);
        buf.put_u8(dev.configuration_value);
        buf.put_u8(dev.num_configuration);
        buf.put_u8(dev.num_interfaces);
    }

    fn encode_interface_info(iface: &InterfaceInfo, buf: &mut BytesMut) {
        buf.put_u8(iface.interface_class);
        buf.put_u8(iface.interface_subclass);
        buf.put_u8(iface.interface_protocol);
        buf.put_u8(0); // padding
    }

    fn encode_urb_header(seqnum: u32, devnum: u32, ep: EndpointAddress, buf: &mut BytesMut) {
        buf.put_u32(seqnum);
        buf.put_u32(devnum);
        buf.put_u32(if ep.direction() == UsbDirection::Out { 0 } else { 1 });
        buf.put_u32(ep.number() as u32);
    }
}

impl Decoder for UsbIpCodec {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut c = Cursor::new(src.clone());

        if c.remaining() < 4 {
            return Ok(None);
        }

        //println!("decoding {:02x?}", &src[..]);

        let op = c.get_u32();

        let item = match op {
            OP_REQ_DEVLIST => {
                if c.remaining() < 4 {
                    return Ok(None);
                }

                c.get_u32(); // status (unused)

                src.advance(c.position() as usize);

                Request::DevList
            },
            OP_REQ_IMPORT => {
                if c.remaining() < 4 + 32 {
                    return Ok(None);
                }

                c.get_u32(); // status (unused)

                let mut busname = [0u8; 32];
                c.copy_to_slice(&mut busname);
                let busname = String::from_utf8_lossy(&busname).trim_end_matches('\0').to_string();

                src.advance(c.position() as usize);

                Request::Import(busname)
            },
            OP_CMD_SUBMIT => {
                if c.remaining() < Self::PDU_LENGTH - 4 {
                    return Ok(None);
                }

                let (seqnum, devid, ep) = Self::decode_urb_header(&mut c)?;

                let transfer_flags = c.get_u32();
                let transfer_buffer_length = c.get_u32();
                let start_frame = c.get_u32();
                let number_of_packets = c.get_u32();
                let interval = c.get_u32();

                let mut setup = [0u8; 8];
                c.copy_to_slice(&mut setup);

                let setup = if setup.iter().any(|&b| b != 0x00) {
                    Some(setup)
                } else {
                    None
                };

                assert!(c.position() == Self::PDU_LENGTH as u64);

                src.advance(Self::PDU_LENGTH);

                let data = if ep.direction() == UsbDirection::Out {
                    if c.remaining() < transfer_buffer_length as usize {
                        return Ok(None);
                    }

                    src.split_to(transfer_buffer_length as usize).into()
                } else {
                    BytesMut::new()
                };

                Request::Submit(SubmitRequest {
                    seqnum,
                    devid,
                    ep,
                    transfer_flags,
                    transfer_buffer_length,
                    start_frame,
                    number_of_packets,
                    interval,
                    setup,
                    data,
                })
            },
            OP_CMD_UNLINK => {
                if c.remaining() < Self::PDU_LENGTH - 4 {
                    return Ok(None);
                }

                let (seqnum, devid, ep) = Self::decode_urb_header(&mut c)?;

                let unlink_seqnum = c.get_u32();

                src.advance(Self::PDU_LENGTH as usize);

                Request::Unlink(UnlinkRequest {
                    seqnum,
                    devid,
                    ep,
                    unlink_seqnum,
                })
            },
            _ => {
                return Err(invalid_data());
            }
        };

        //println!("Recv: {:?}", &item);

        return Ok(Some(item));
    }
}

impl Encoder<Response> for UsbIpCodec {
    type Error = io::Error;

    fn encode(&mut self, msg: Response, buf: &mut BytesMut) -> Result<(), Self::Error> {
        //println!("Send: {:?}", &msg);

        match msg {
            Response::DevList(devices) => {
                buf.reserve(
                    (3 * 4)
                    + devices.iter()
                        .map(|d|
                            Self::DEVICE_INFO_SIZE
                            + d.interfaces.len() * Self::INTERFACE_INFO_SIZE)
                        .sum::<usize>());

                buf.put_u32(OP_REP_DEVLIST); // version, reply code
                buf.put_u32(0); // status (operation cannot fail)
                buf.put_u32(devices.len() as u32);

                for dev in devices {
                    Self::encode_device_info(&dev.device, buf);

                    for iface in &dev.interfaces {
                        Self::encode_interface_info(&iface, buf)
                    }
                }
            },
            Response::Import(res) => {
                buf.reserve(
                    (2 * 4)
                    + if res.device.is_some() { Self::DEVICE_INFO_SIZE } else { 0 }
                );

                buf.put_u32(OP_REP_IMPORT); // version, reply code
                buf.put_u32(res.status);

                if let Some(dev) = res.device {
                    Self::encode_device_info(&dev, buf);
                }
            },
            Response::Submit(res) => {
                let data_len = res.data.len();

                buf.reserve(4 + Self::URB_HEADER_SIZE + (5 * 4) + 8 + data_len);

                buf.put_u32(OP_RET_SUBMIT);

                Self::encode_urb_header(res.seqnum, res.devid, res.ep, buf);

                buf.put_u32(res.status);
                buf.put_u32(data_len as u32); // actual_length
                buf.put_u32(res.actual_start_frame);
                buf.put_u32(res.number_of_packets);
                buf.put_u32(res.error_count);

                //buf.put_slice(&res.setup.unwrap_or([0u8; 8]));
                buf.put_u64(0); // SETUP

                assert!(buf.len() == Self::PDU_LENGTH);

                buf.put_slice(&res.data);
            },
            Response::Unlink(res) => {
                buf.reserve(4 + Self::URB_HEADER_SIZE + 4);

                buf.put_u32(OP_RET_UNLINK);

                Self::encode_urb_header(res.seqnum, res.devid, res.ep, buf);
                buf.put_u32(res.status);

                while buf.len() < Self::PDU_LENGTH {
                    buf.put_u8(0);
                }
            },
        }

        Ok(())
    }
}
