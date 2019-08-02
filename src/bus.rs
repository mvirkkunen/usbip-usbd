use std::collections::{VecDeque, LinkedLIst};
use std::sync::Arc;
use bytes::Bytes;
use tokio::sync::lock::Lock;
use tokio::sync::mpsc::UnboundedSender<Urb>;
use usb_device::{
    Result, UsbError, UsbDirection,
    class::UsbClass,
    endpoint::{EndpointType, EndpointAddress},
    bus::{UsbBusAllocator, PollResult},
};
use crate::device::*;

pub const NUM_ENDPOINTS: usize = 16;

// TODO: Handle different transfer types differently!

pub struct UsbBus {
    urb_queue: Arc<Mutex<LinkedList<Urb>>>,
    ep_in: [Enpdoint; NUM_ENDPOINTS],
    ep_out: [Enpdoint; NUM_ENDPOINTS],
}

/// Virtual USB peripheral driver
impl UsbBus {
    pub(crate) fn new(device: Arc<Device>) -> UsbBusAllocator<UsbBus> {
        UsbBusAllocator::new(UsbBus {
            device,
            ep_in: Default::default(),
            ep_out: Default::default(),
        })
    }
}

impl UsbBus {
    pub fn register_class<T: UsbClass<Self>>(&self, class: Arc<Lock<T>>) {
        self.device.register_class(

    }

    pub fn events(&self) -> Events {
        self.device.events()
    }
    
    fn take_next_urb(&self, ep_addr: EndpointAddress) -> Option<Urb> {
        let mut urb_queue = self.urb_queue.lock().unwrap();

        match usb_queue.iter()
            .enumerate()
            .find(|u| u.ep == ep_addr)
        {
            Some((index, urb)) => {
                urb_queue.remove(index);
                Some(urb),
            },
            None => None,
        }
    }

    fn complete_urb(&self, urb: Urb) {
        unimplemented!();
    }
}

impl usb_device::bus::UsbBus for UsbBus {
    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        interval: u8) -> Result<EndpointAddress>
    {
        unimplemented!();
    }

    fn enable(&mut self) {
        // nop
    }

    fn reset(&self) {
        for ep in &self.ep_in.concat(&self.ep_out) {
            let state = ep.state.lock().unwrap();

            state.current_urb = None;
        }
    }

    fn set_device_address(&self, _addr: u8) {
        // nop
    }

    fn poll(&self) -> PollResult {
        unimplemented!();
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize> {
        if ep_addr.direction() != UsbDirection::Out {
            return Err(UsbError::InvalidEndpoint);
        }

        let ep = &self.endpoints[ep_addr.index()];
        let ep_type = ep.ep_type.ok_er(UsbError::InvalidEndpoint)?;

        if buf.len() > ep.max_packet_size {
            return Err(UsbError::BufferOverflow);
        }

        // Just write packets to the Server

        let mut state = ep.state.lock().unwrap();

        let buf_option = Some(buf);

        if state.urb.is_none() {
            state.urb = self.take_next_urb(ep_addr);

            if let Some(b) = state.leftover.take() {
                urb.data.extend_from_slice(&b);
            }

            if state.urb.is_none() {
                return Err(UsbError::WouldBlock);
            }
        }

        urb.data.extend_from_slice(buf);

        if urb.data.len() > urb.len {
            self.leftover = Some(urb.data.split_off(urb.len));
        }

        // TODO: Is this correct?
        if urb.data.len() == urb.len || buf.len() < ep.max_packet_size {
            self.complete_urb(state.urb.take());
        }

        Ok(buf.len())
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        if ep_addr.direction() != UsbDirection::Out {
            return Err(UsbError::InvalidEndpoint);
        }

        let ep = &self.endpoints[ep_addr.index()];

        if ep.ep_type.is_none() {
            return Err(UsbError::InvalidEndpoint);
        }
        
        let mut state = ep.state.lock().unwrap();

        if state.urb.is_none() {
            state.urb = self.take_next_urb(ep_addr);

            if state.urb.is_none() {
                return Err(UsbError::WouldBlock);
            }
        }

        if state.buffer.is_empty() {
            
        }

        match packets.get(0) {
            Some(bytes) => {
                if buf.len() < bytes.len() {
                    return Err(UsbError::BufferOverflow);
                }

                buf[..bytes.len()].copy_from_slice(&bytes);

                buf.pop_front();

                Ok(bytes.len())
            },
            None => {
                return Err(UsbError::WouldBlock);
            }
        }
    }

    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {

    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        unimplemented!();
    }

    fn suspend(&self) {

    }

    fn resume(&self) {

    }

    fn force_reset(&self) -> Result<()> {
        Err(UsbError::Unsupported)
    }
}

struct Endpoint {
    pub max_packet_size: usize,
    pub ep_type: Option<EndpointType>,
    pub state: Mutex<EndpointState>,
}

struct EndpointState {
    pub leftover: Option<BytesMut>,
    pub urb: Option<Urb>,
}

struct Urb {
    seqnum: u32,
    devid: u32,
    ep: EndpointAddress,
    len: usize,
    setup: Option<[u8; 8]>,
    data: BytesMut,
}