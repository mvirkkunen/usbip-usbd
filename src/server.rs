use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use bytes::{Bytes, BytesMut};
use tokio::prelude::*;
use tokio::sync::{mpsc, watch};
use tokio::net::{TcpListener, TcpStream};
use usb_device::UsbDirection;
use usb_device::allocator::UsbAllocator;
use usb_device::control::Request;
use usb_device::endpoint::EndpointAddress;
use crate::bus::UsbBus;
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
    buses: HashMap<u32, BusShared>,
}

impl Client {
    fn new(stream: TcpStream) -> Self {
        Client {
            stream,
            next_devid: 1,
            buses: HashMap::new(),
        }
    }

    pub fn attach(&mut self, bus_id: &str) -> UsbAllocator<UsbBus> {
        //let device = Arc::new(Device::new());

        //self.shared.devices.write().unwrap().push(Arc::clone(&device));

        let devid = self.next_devid;
        self.next_devid += 1;

        let mut shared = BusShared::new(devid, bus_id);

        let bus = UsbBus::new(shared.channel());

        self.buses.insert(devid, shared);

        bus
    }

    pub async fn run(mut self) -> tokio::io::Result<()> {
        let mut framed = tokio::codec::Framed::new(self.stream, UsbIpCodec);

        println!("reading a thing");

        while let Some(packet) = framed.next().await {
            let packet = match packet {
                Ok(p) => p,
                Err(err) => {
                    println!("Error: {:?}", err);
                    return Err(err);
                }
            };

            match packet {
                Request::DevList => {
                    let mut devices = Vec::new();

                    for bus in self.buses.values_mut() {
                        devices.push(bus.enumerate().await);
                    }

                    framed.send(Response::DevList(devices)).await;
                },
                _ => { }
            }

            println!("{:?}", packet);
        }

        Ok(())
    }
}

pub struct BusShared {
    devid: u32,
    bus_id: String,
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    complete_sender: mpsc::UnboundedSender<Urb>,
    complete_receiver: mpsc::UnboundedReceiver<Urb>,
    poll_sender: watch::Sender<()>,
    poll_receiver: watch::Receiver<()>,
}

impl BusShared {
    pub fn new(devid: u32, bus_id: &str) -> Self {
        let (complete_sender, complete_receiver) = mpsc::unbounded_channel();
        let (poll_sender, poll_receiver) = watch::channel(());

        BusShared {
            devid,
            bus_id: bus_id.to_owned(),
            urb_queue: Arc::new(Mutex::new(VecDeque::new())),
            complete_sender,
            complete_receiver,
            poll_sender,
            poll_receiver,
        }
    }

    pub fn channel(&mut self) -> BusChannel {
        BusChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            complete_sender: self.complete_sender.clone(),
            poll_receiver: self.poll_receiver.clone(),
        }
    }

    pub fn submit_urb(&mut self, urb: Urb) {
        // Control transfers must always first be directed to the control OUT endpoint for SETUP

        if urb.control.is_some() {
            urb.ep = EndpointAddress::from_parts(urb.req_ep.number(), UsbDirection::Out);
        }

        self.urb_queue.lock().unwrap().push_back(urb);
        self.poll_sender.broadcast(());
    }

    pub async fn enumerate(&mut self) -> DeviceInterfaceInfo {
        

        while let Some(urb) = self.complete_receiver.recv().await {
            if !urb.internal {
                continue;
            }

            println!("internal complete 2: {:?}", urb);
        }

        panic!("rip");
    }

    async fn control_transfer(&mut self, req: Request) -> Result<Option<Bytes>, ()> {
        self.submit_urb(Urb {
            seqnum: 0,
            devid: 0,
            ep: EndpointAddress::from_parts(0, req.direction),
            req_ep: EndpointAddress::from_parts(0, req.direction),
            len: req.length as usize,
            control: Some(Control {
                setup: [
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
                ],
                state: ControlState::Setup,
            }),
            data: BytesMut::new(),
            internal: true,
        });

        while let Some(urb) = self.complete_receiver.recv().await {
            if !urb.internal {
                continue;
            }

            return 
        }
    }
}

pub struct BusChannel {
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    complete_sender: mpsc::UnboundedSender<Urb>,
    poll_receiver: watch::Receiver<()>,
}

impl BusChannel {
    pub fn clone(&self) -> BusChannel {
        BusChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            complete_sender: self.complete_sender.clone(),
            poll_receiver: self.poll_receiver.clone(),
        }
    }

    pub fn take_next_urb(&mut self, ep_addr: EndpointAddress) -> Option<Urb> {
        let mut queue = self.urb_queue.lock().unwrap();

        match queue.iter()
            .enumerate()
            .find(|u| u.1.ep == ep_addr)
        {
            Some((index, _)) => {
                queue.remove(index)
            },
            None => None,
        }
    }

    pub fn complete_urb(&mut self, mut urb: Urb) {
        if urb.setup.is_some() && urb.ep.direction() == UsbDirection::In {
            // This URB was for a CONTROL IN transfer, pass it on to the OUT endpoint for the
            // response

            urb.setup = None;
            urb.ep = EndpointAddress::from_parts(urb.ep.number(), UsbDirection::Out);

            self.urb_queue.lock().unwrap().push_front(urb);
        } else {
            match self.complete_sender.try_send(urb) {
                Ok(_) => {},
                Err(_) => {
                    panic!("try_send failed");
                }
            };
        }
    }

    pub fn poller(&self) -> watch::Receiver<()> {
        self.poll_receiver.clone()
    }
}

#[derive(Debug)]
pub struct Urb {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress, // current endpoint processing this URB
    pub req_ep: EndpointAddress, // request endpoint, could be different for control URBs
    pub len: usize,
    pub control: Option<Control>,
    pub data: BytesMut,
    pub internal: bool,
}

#[derive(Debug)]
pub struct Control {
    pub setup: [u8; 8],
    pub state: ControlState,
}

#[derive(Eq, PartialEq, Debug)]
pub enum ControlState {
    Setup,
    DataIn,
    StatusOut
    DataOut,
    StatusIn,
    Complete,
}