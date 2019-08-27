use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::prelude::*;
use tokio::net::TcpListener;
use usb_device::bus::UsbBusAllocator;
use crate::bus::{UsbBus, Urb};
use crate::device::Device;

struct Connection<'a> {
    stream: TcpStream,
    devices: Vec<&'a UsbBus>,
}

impl Connection {
    fn new(&self, stream: TcpStream) -> self {
        Connection {
            stream,
        }
    }

    pub fn attach(&mut self, bus_id: &str) -> UsbBusAllocator<Box<Pin<UsbBus>>> {
        //let device = Arc::new(Device::new());

        //self.shared.devices.write().unwrap().push(Arc::clone(&device));

        UsbBus::new(bus_id)
    }

    pub fn complete_urb(&self) {
        
    }
}

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn bind(addr: &SocketAddr) -> io::Result<Server> {
        let listener = TcpListener::bind(addr)?;

        Ok(Server { listener })
    }

    pub async fn accept(&mut self) -> impl Future<Output = io::Result<Connection>> {
        let stream = self.listener.accept()?;

        Connection::new(stream)
    }
}