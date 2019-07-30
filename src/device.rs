use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use tokio::prelude::*;
use crate::bus::NUM_ENDPOINTS;

pub struct Device {
    enumerated: AtomicBool,
    suspended: AtomicBool,
    endpoint_in: [Mutex<Endpoint>; NUM_ENDPOINTS],
    endpoint_out: [Mutex<Endpoint>; NUM_ENDPOINTS],
}

impl Device {
    pub fn new() -> Device {
        Device {
            enumerated: AtomicBool::new(false),
            suspended: AtomicBool::new(true),
            endpoint_in: Default::default(),
            endpoint_out: Default::default(),
        }
    }

    pub fn submit(urb: Urb) -> Submit {

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

struct Urb {
    started: bool,
    data: Vec<u8>,
}