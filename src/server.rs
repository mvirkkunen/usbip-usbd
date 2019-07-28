use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::prelude::*;
use tokio::net::{TcpListener, Incoming};
use usb_device::bus::UsbBusAllocator;
use crate::bus::UsbBus;
use crate::device::Device;

pub struct Server {
    shared: Arc<Shared>,
}

struct Shared {
    devices: RwLock<Vec<Arc<Device>>>,
}

impl Server {
    pub fn bind(addr: &SocketAddr) -> io::Result<Server> {
        let listener = TcpListener::bind(addr)?;

        let shared = Arc::new(Shared {
            devices: RwLock::new(Vec::new()),
        });

        listener.incoming()
            .for_each(|sock| {
                Ok(())
            });

        Ok(Server {
            shared,
        })
    }

    pub fn attach(&self, bus_id: &str) -> (Events, UsbBusAllocator<UsbBus>) {
        let device = Arc::new(Device::new());

        self.shared.devices.write().unwrap().push(Arc::clone(&device));

        (Events {}, UsbBus::new(device))
    }
}

pub struct Events {

}

impl Stream for Events {
    type Item = ();

    type Error = ();

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        unimplemented!();
    }
}