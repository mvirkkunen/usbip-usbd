use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::prelude::*;
use tokio::sync::lock::Lock;
use usb_device::class::UsbClass;
use crate::bus::NUM_ENDPOINTS;

pub struct Device {
    enumerated: AtomicBool,
    suspended: AtomicBool,
    //endpoint_in: [Mutex<Endpoint>; NUM_ENDPOINTS],
    //endpoint_out: [Mutex<Endpoint>; NUM_ENDPOINTS],
}

impl Device {
    pub fn new() -> Device {
        Device {
            enumerated: AtomicBool::new(false),
            suspended: AtomicBool::new(true),
            //endpoint_in: Default::default(),
            //endpoint_out: Default::default(),
        }
    }

    pub fn submit(&self, urb: Urb) -> Submit {

    }

    pub fn register_class(&self, class: Arc<Lock<&dyn UsbClass>>) {

    }
    
    pub fn events(&self) -> Events {

    }
}

pub struct Submit {

}

impl Future for Submit {
    type Item = ();

    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        unimplemented!();
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

struct Endpoint {
    stalled: bool,
    urbs: Vec<Urb>,
}

impl Default for Endpoint {
    fn default() -> Endpoint {
        Endpoint {
            stalled: false,
            urbs: Vec::new(),
        }
    }
}
