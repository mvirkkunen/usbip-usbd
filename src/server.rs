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

    pub fn attach(&self, bus_id: &str) -> UsbBusAllocator<UsbBus> {
        let device = Arc::new(Device::new());

        self.shared.devices.write().unwrap().push(Arc::clone(&device));

        UsbBus::new(device)
    }
}
