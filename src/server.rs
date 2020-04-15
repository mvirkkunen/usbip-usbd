use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{
    Arc, Mutex,
    atomic::{
        AtomicBool,
        Ordering::SeqCst,
    }
};
use bytes::{Bytes, BytesMut, Buf};
use futures::sink::SinkExt as _;
use futures::stream::StreamExt as _;
//use futures_codec::Framed;
//use tokio::prelude::*;
//use tokio::stream::StreamExt as _;
use tokio::sync::{mpsc, watch};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use usb_device::{
    UsbDirection,
    control,
    descriptor::descriptor_type,
    endpoint::EndpointAddress,
};
use crate::usbcore::UsbCore;
use crate::protocol::*;

pub struct Server {
    listener: TcpListener,
}

impl Server {
    // TODO: Use ToSocketAddrs
    pub async fn bind(addr: &str) -> io::Result<Server> {
        let listener = TcpListener::bind(addr).await?;

        Ok(Server { listener })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, io::Error> {
        self.listener.local_addr()
    }

    pub async fn accept(&mut self) -> io::Result<Client> {
        self.listener.accept().await.map(|(stream, _)| Client::new(stream))
    }
}

pub struct Client {
    stream: TcpStream,
    next_devid: u32,
    cores: HashMap<u32, ClientCore>,
    complete_sender: mpsc::UnboundedSender<Urb>,
    complete_receiver: mpsc::UnboundedReceiver<Urb>,
}

impl Client {
    fn new(stream: TcpStream) -> Self {
        let (complete_sender, complete_receiver) = mpsc::unbounded_channel();

        Client {
            stream,
            next_devid: 1,
            cores: HashMap::new(),
            complete_sender,
            complete_receiver,
        }
    }

    pub fn attach(&mut self, bus_id: &str) -> (UsbCore, Poller) {
        //let devid = self.next_devid;
        let devid = 65537; // ????
        //self.next_devid += 1;

        let (ccore, poller) = ClientCore::new(devid, bus_id, self.complete_sender.clone());

        let usbcore = UsbCore::new(ccore.channel.clone());

        self.cores.insert(devid, ccore);

        (usbcore, poller)
    }

    pub async fn run(mut self) -> io::Result<()> {
        let (sink, mut stream) = Framed::new(self.stream, UsbIpCodec::new()).split();
        let sink = Arc::new(tokio::sync::Mutex::new(sink));

        let mut complete_receiver = self.complete_receiver;

        let csink = Arc::clone(&sink);

        /*for core in self.cores.values_mut() {
            core.enumerate().await.expect("enumeration failed");
        }*/

        tokio::spawn(async move {
            while let Some(urb) = complete_receiver.recv().await {
                if urb.internal {
                    println!("completed internal urb: {:?}", urb);
                    continue;
                }

                csink.lock().await.send(
                    Response::Submit(
                        SubmitResponse {
                            seqnum: urb.seqnum,
                            devid: urb.devid,
                            ep: urb.req_ep,
                            status: 0, // OK
                            actual_length: urb.data.len() as u32,
                            actual_start_frame: 0,
                            number_of_packets: 0,
                            error_count: 0,
                            setup: None,
                            data: Some(urb.data.into()),
                        })).await.expect("send failed");
            }
        });

        /*let mut devices = Vec::new();
        for core in self.cores.values_mut() {
            devices.push(core.enumerate().await);
        }*/

        while let Some(packet) = stream.next().await {
            let packet = packet?;

            match packet {
                Request::DevList => {
                    let mut devices = Vec::new();

                    for core in self.cores.values_mut() {
                        devices.push(core.enumerate().await.expect("enumeration failed"));
                    }

                    sink.lock().await.send(Response::DevList(devices)).await?;
                },
                Request::Import(bus_id) => {
                    println!("IMPORT {}", bus_id);

                    match self.cores.values_mut().find(|c| c.bus_id == bus_id) {
                        Some(core) => {
                            let info = core.enumerate().await.expect("enumeration failed");

                            sink.lock().await.send(
                                Response::Import(
                                    ImportResponse {
                                        status: 0, // OK
                                        device: Some(Arc::clone(&info.device)),
                                    })).await.expect("send failed");
                        },
                        None => {
                            sink.lock().await.send(
                                Response::Import(
                                    ImportResponse {
                                        status: 1, // ERROR
                                        device: None,
                                    })).await.expect("send failed");
                        },
                    };
                    // TODO
                },
                Request::Submit(req) => {
                    if let Some(core) = self.cores.values_mut().find(|c| c.devid == req.devid) {
                        let control = req.setup.map(|setup| UrbControl {
                            setup,
                            state: ControlState::Setup,
                        });

                        let ep = if control.is_some() {
                            EndpointAddress::from_parts(0, UsbDirection::Out)
                        } else {
                            req.ep
                        };

                        core.submit_urb(Urb {
                            seqnum: req.seqnum,
                            devid: req.devid,
                            ep,
                            req_ep: req.ep,
                            control,
                            len: req.transfer_buffer_length as usize,
                            data: BytesMut::new(),
                            internal: false,
                        });
                    } else {
                        sink.lock().await.send(
                            Response::Submit(
                                SubmitResponse {
                                    seqnum: req.seqnum,
                                    devid: req.devid,
                                    ep: req.ep,
                                    status: 1, // ERROR
                                    actual_length: 0,
                                    actual_start_frame: 0,
                                    number_of_packets: 0,
                                    error_count: 0,
                                    setup: None,
                                    data: None,
                                })).await.expect("send failed");
                    }
                },
                Request::Unlink(req) => {
                    let success = self.cores.values_mut()
                        .find(|c| c.devid == req.devid)
                        .map(|c| c.unlink_urb(req.unlink_seqnum))
                        .unwrap_or(false);

                    sink.lock().await.send(
                        Response::Unlink(
                            UnlinkResponse {
                                seqnum: req.seqnum,
                                devid: req.devid,
                                ep: req.ep,
                                status: if success { 1 } else { 0 },
                                unlink_seqnum: req.unlink_seqnum,
                            })).await.expect("send failed");
                },
                _ => { }
            }

            //self.request_poll_sender.send(()).unwrap();
        }

        Ok(())
    }
}

pub struct Poller(watch::Receiver<()>);

impl Poller {
    pub async fn poll(&mut self) {
        self.0.recv().await;
    }
}

pub struct ClientCore {
    devid: u32,
    bus_id: String,
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    poll_sender: watch::Sender<()>,
    channel: CoreChannel,
    info: Option<Arc<DeviceInterfaceInfo>>,
}

impl ClientCore {
    pub fn new(devid: u32, bus_id: &str, complete_sender: mpsc::UnboundedSender<Urb>)
        -> (Self, Poller)
    {
        let (poll_sender, poll_receiver) = watch::channel(());

        let urb_queue = Arc::new(Mutex::new(VecDeque::new()));

        (
            ClientCore {
                devid,
                bus_id: bus_id.to_owned(),
                urb_queue: Arc::clone(&urb_queue),
                poll_sender,
                info: None,
                channel: CoreChannel {
                    urb_queue,
                    complete_sender,
                    internal_complete_sender: Arc::new(Mutex::new(None)),
                    control_in_progress: Arc::new(AtomicBool::new(false)),
                }
            },
            Poller(poll_receiver),
        )
    }

    pub fn submit_urb(&mut self, urb: Urb) {
        println!("submit: {:?}", &urb);

        // Control transfers must always first be directed to the control OUT endpoint for SETUP
        /*if urb.is_control {
            urb.ep = EndpointAddress::from_parts(urb.req_ep.number(), UsbDirection::Out);
        }*/

        self.urb_queue.lock().unwrap().push_back(urb);
        self.poll_sender.broadcast(()).expect("poll send failed");
    }

    pub fn unlink_urb(&mut self, seqnum: u32) -> bool {
        let mut queue = self.urb_queue.lock().unwrap();
        let len_before = queue.len();

        queue.retain(|u| u.seqnum == seqnum);

        queue.len() != len_before
    }

    pub async fn enumerate(&mut self) -> Result<Arc<DeviceInterfaceInfo>, String> {
        if let Some(info) = self.info.as_ref() {
            return Ok(Arc::clone(info));
        }

        println!("get device descriptor");
        let mut dev = self.get_descriptor(descriptor_type::DEVICE, 0, 18).await?;
        println!("{:02x?}", dev);

        if usize::from(dev.get_u8()) < 18 {
            return Err("invalid device descriptor: length field too small".into());
        }

        if dev.get_u8() != descriptor_type::DEVICE {
            return Err("invalid device descriptor: incorrect descriptor type".into());
        }

        dev.advance(2); // bcdUSB
        let device_class = dev.get_u8();
        let device_subclass = dev.get_u8();
        let device_protocol = dev.get_u8();
        dev.advance(1); // bMaxPacketSize0
        let id_vendor = dev.get_u16_le();
        let id_product = dev.get_u16_le();
        let bcd_device = dev.get_u16_le();
        dev.advance(3); // iManufacturer, iProduct, iSerialNUmber
        let num_configuration = dev.get_u8();

        self.control_transfer(control::Request {
            direction: UsbDirection::Out,
            request_type: control::RequestType::Standard,
            recipient: control::Recipient::Device,
            request: control::Request::SET_ADDRESS,
            value: 1,
            index: 0,
            length: 0,
        }).await?;

        println!("get configuration descriptor");
        let mut config_all = self.get_descriptor(descriptor_type::CONFIGURATION, 0, 9).await?;
        println!("{:02x?}", config_all);

        let len = config_all.len();

        let mut config = config_all.split_to(9);

        if usize::from(config.get_u8()) < 9 {
            return Err("invalid configuration descriptor: length field too small".into());
        }

        if config.get_u8() != descriptor_type::CONFIGURATION {
            return Err("invalid configuration descriptor: incorrect descriptor type".into());
        }

        if usize::from(config.get_u16_le()) != len {
            return Err("invalid configuration descriptor: wTotalLength mismatch".into());
        }

        let num_interfaces = config.get_u8();
        let configuration_value = config.get_u8();

        let mut interfaces = Vec::new();

        while !config_all.is_empty() {
            if config_all.len() < 2 {
                return Err("invalid configuration descriptor: truncated".into());
            }

            let len = usize::from(config_all.get_u8());
            let dtype = config_all.get_u8();

            let mut desc = config_all.split_to(len - 2);

            if dtype == descriptor_type::INTERFACE {
                if desc.len() < 7 {
                    return Err("invalid interface descriptor: too short".into());
                }

                desc.advance(3); // bInterfaceNumber, bAlternateSetting, bNumEndpoints

                interfaces.push(InterfaceInfo {
                    interface_class: desc.get_u8(),
                    interface_subclass: desc.get_u8(),
                    interface_protocol: desc.get_u8(),
                });
            }
        }

        let info = Arc::new(DeviceInterfaceInfo {
            device: Arc::new(DeviceInfo {
                path: String::from("/virtual"),
                busid: self.bus_id.clone(),
                busnum: 1,
                devnum: self.devid,
                device_class,
                device_subclass,
                device_protocol,
                speed: 2, // USB_SPEED_FULL
                id_vendor,
                id_product,
                bcd_device,
                configuration_value,
                num_configuration,
                num_interfaces,
            }),
            interfaces,
        });

        self.info = Some(Arc::clone(&info));

        Ok(info)
    }

    async fn get_descriptor(&mut self, dtype: u8, dindex: u8, min_len: usize)
        -> Result<Bytes, String>
    {
        let req = control::Request {
            direction: UsbDirection::In,
            request_type: control::RequestType::Standard,
            recipient: control::Recipient::Device,
            request: control::Request::GET_DESCRIPTOR,
            value: (u16::from(dtype) << 8) | u16::from(dindex),
            index: 0,
            length: 0xffff,
        };

        let desc = self.control_transfer(req).await?;
        if desc.len() < min_len {
            return Err(format!("invalid {} descriptor: data length too short", dtype));
        }

        Ok(desc)
    }

    async fn control_transfer(&mut self, req: control::Request)
        -> Result<Bytes, String>
    {
        let setup = [
            // bmRequestType
            (req.direction as u8) | ((req.request_type as u8) << 5) | (req.recipient as u8),
            // bRequest
            req.request,
            // wValue
            req.value as u8, (req.value >> 8) as u8,
            // wIndex
            req.index as u8, (req.index >> 8) as u8,
            // wLength
            req.length as u8, (req.length >> 8) as u8,
        ];

        self.submit_urb(Urb {
            seqnum: 0,
            devid: 0,
            ep: EndpointAddress::from_parts(0, UsbDirection::Out),
            req_ep: EndpointAddress::from_parts(0, UsbDirection::In),
            len: 0xffff,
            control: Some(
                UrbControl {
                    setup,
                    state: ControlState::Setup,
                }
            ),
            data: BytesMut::new(),
            internal: true,
        });

        let (sender, mut receiver) = mpsc::unbounded_channel();

        *self.channel.internal_complete_sender.lock().unwrap() = Some(sender);

        let urb = receiver.recv().await.ok_or("recv failed")?;

        *self.channel.internal_complete_sender.lock().unwrap() = None;

        Ok(urb.data.into())
    }
}

// The Arc/Mutex mess is probably backwards
pub struct CoreChannel {
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    complete_sender: mpsc::UnboundedSender<Urb>,
    internal_complete_sender: Arc<Mutex<Option<mpsc::UnboundedSender<Urb>>>>,
    // TODO: Make this per endpoint or something
    control_in_progress: Arc<AtomicBool>,
}

impl CoreChannel {
    pub fn take_next_urb(&mut self, ep_addr: EndpointAddress) -> Option<Urb> {
        let mut queue = self.urb_queue.lock().unwrap();

        match queue.iter()
            .enumerate()
            .find(|u| u.1.ep == ep_addr)
        {
            Some((index, urb)) => {
                if let Some(control) = &urb.control {
                    if self.control_in_progress.load(SeqCst) {
                        if control.state == ControlState::Setup {
                            return None;
                        }
                    } else {
                        self.control_in_progress.store(true, SeqCst);
                    }
                }

                queue.remove(index)
            },
            None => None,
        }
    }

    pub fn complete_urb(&mut self, mut urb: Urb) {
        if let Some(ref mut control) = urb.control {
            match control.state {
                ControlState::Setup => panic!("Cannot complete_urb a SETUP"),

                ControlState::Data => {
                    // OUT endpoint is passing to IN endpoint

                    urb.ep = EndpointAddress::from_parts(urb.ep.number(), UsbDirection::In);

                    self.urb_queue.lock().unwrap().push_front(urb);
                    return;
                },

                ControlState::Status => {
                    let status_dir = match urb.req_ep.direction() {
                        UsbDirection::Out => UsbDirection::In,
                        UsbDirection::In => UsbDirection::Out,
                    };

                    println!("passing to {:?}", status_dir);

                    urb.ep = EndpointAddress::from_parts(urb.ep.number(), status_dir);

                    self.urb_queue.lock().unwrap().push_front(urb);
                    return;
                },

                ControlState::Complete => {
                    /* handled below */

                    self.control_in_progress.store(false, SeqCst);
                }
            }
        }

        if let Some(sender) = self.internal_complete_sender.lock().unwrap().take() {
            sender.send(urb).unwrap();
        } else {
            self.complete_sender.send(urb).unwrap();
        }
    }
}

impl Clone for CoreChannel {
    fn clone(&self) -> CoreChannel {
        CoreChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            complete_sender: self.complete_sender.clone(),
            internal_complete_sender: Arc::clone(&self.internal_complete_sender),
            control_in_progress: Arc::clone(&self.control_in_progress),
        }
    }
}

#[derive(Debug)]
pub struct Urb {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress, // current endpoint processing this URB
    pub req_ep: EndpointAddress, // request endpoint, could be different for control URBs
    pub len: usize,
    pub control: Option<UrbControl>,
    pub data: BytesMut,
    pub internal: bool,
}

#[derive(Debug)]
pub struct UrbControl {
    pub setup: [u8; 8],
    pub state: ControlState,
}

#[derive(Eq, PartialEq, Debug)]
pub enum ControlState {
    Setup,
    Data,
    Status,
    Complete,
}
