use std::io;
use std::net::SocketAddr;
use tokio::prelude::*;
use tokio::net::{TcpListener, TcpStream};
use usb_device::allocator::UsbAllocator;
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
    //devices: Vec<&'a UsbBus>,
}

impl Client {
    fn new(stream: TcpStream) -> Self {
        Client {
            stream,
        }
    }

    pub fn attach(&mut self, bus_id: &str) -> UsbAllocator<UsbBus> {
        //let device = Arc::new(Device::new());

        //self.shared.devices.write().unwrap().push(Arc::clone(&device));

        UsbBus::new(bus_id)
    }

    pub async fn run(self) -> tokio::io::Result<()> {
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
        }

        Ok(())
    }
}