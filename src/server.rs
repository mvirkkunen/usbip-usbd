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
use bytes::{Bytes, BytesMut};
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
    descriptor,
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
        let devid = self.next_devid;
        self.next_devid += 1;

        let (ccore, poller) = ClientCore::new(devid, bus_id, self.complete_sender.clone());

        let usbcore = UsbCore::new(ccore.channel.clone());

        self.cores.insert(devid, ccore);

        (usbcore, poller)
    }

    pub async fn run(mut self) -> io::Result<()> {
        let (sink, mut stream) = Framed::new(self.stream, UsbIpCodec).split();
        let sink = Arc::new(tokio::sync::Mutex::new(sink));

        let mut complete_receiver = self.complete_receiver;

        let csink = Arc::clone(&sink);

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
                            //status: urb.status,
                            status: 0, // TODO
                            actual_length: urb.data.len() as u32,
                            actual_start_frame: 0,
                            number_of_packets: 0,
                            error_count: 0,
                            setup: None,
                            data: Some(urb.data.into()),
                        })).await.expect("send failed");
            }
        });

        self.cores.values_mut().next().unwrap().enumerate().await;

        while let Some(packet) = stream.next().await {
            let packet = packet?;

            println!("{:?}", &packet);

            match packet {
                Request::DevList => {
                    let mut devices = Vec::new();

                    for usb in self.cores.values_mut() {
                        devices.push(usb.enumerate().await);
                    }

                    sink.lock().await.send(Response::DevList(devices)).await?;
                },
                Request::Import(bus_id) => {
                    // TODO
                },
                Request::Submit(req) => {
                    // TODO
                },
                Request::Unlink(req) => {
                    // TODO
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
        // Control transfers must always first be directed to the control OUT endpoint for SETUP
        /*if urb.is_control {
            urb.ep = EndpointAddress::from_parts(urb.req_ep.number(), UsbDirection::Out);
        }*/

        self.urb_queue.lock().unwrap().push_back(urb);
        self.poll_sender.broadcast(()).expect("poll send failed");
    }

    pub async fn enumerate(&mut self) -> DeviceInterfaceInfo {
        let device = self.internal_control_transfer(control::Request {
            direction: UsbDirection::In,
            request_type: control::RequestType::Standard,
            recipient: control::Recipient::Device,
            request: control::Request::GET_DESCRIPTOR,
            value: u16::from(descriptor::descriptor_type::DEVICE) << 8,
            index: 0,
            length: 0xffff,
        }).await;

        println!("{:?}", device);

        let config = self.internal_control_transfer(control::Request {
            direction: UsbDirection::In,
            request_type: control::RequestType::Standard,
            recipient: control::Recipient::Device,
            request: control::Request::GET_DESCRIPTOR,
            value: u16::from(descriptor::descriptor_type::CONFIGURATION) << 8,
            index: 0,
            length: 0xffff,
        }).await;

        println!("{:?}", config);

        /*while let Some(urb) = self.complete_receiver.recv().await {
            if !urb.internal {
                continue;
            }

            println!("internal complete 2: {:?}", urb);
        }*/

        panic!("rip");
    }

    async fn internal_control_transfer(&mut self, request: control::Request)
        -> Result<Bytes, ()>
    {
        self.submit_urb(Urb {
            seqnum: 0,
            devid: 0,
            ep: EndpointAddress::from_parts(0, UsbDirection::Out),
            req_ep: EndpointAddress::from_parts(0, request.direction),
            len: request.length as usize,
            control: Some(
                UrbControl {
                    request,
                    state: ControlState::Setup,
                }
            ),
            data: BytesMut::new(),
            internal: true,
        });

        let (sender, mut receiver) = mpsc::unbounded_channel();

        *self.channel.internal_complete_sender.lock().unwrap() = Some(sender);

        println!("begin xfer");
        let urb = receiver.recv().await.ok_or(())?;
        println!("end xfer");

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
                    let status_dir = match control.request.direction {
                        UsbDirection::Out => UsbDirection::In,
                        UsbDirection::In => UsbDirection::Out,
                    };

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
    pub request: control::Request,
    pub state: ControlState,
}

#[derive(Eq, PartialEq, Debug)]
pub enum ControlState {
    Setup,
    Data,
    Status,
    Complete,
}
