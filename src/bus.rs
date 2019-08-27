use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use bytes::{Bytes, BytesMut};
use tokio::sync::Lock;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::watch;
use usb_device::{
    Result, UsbError, UsbDirection,
    class::UsbClass,
    endpoint::{EndpointType, EndpointAddress},
    bus::{UsbBusAllocator, PollResult},
};
use crate::device::*;

pub const NUM_ENDPOINTS: usize = 16;

pub struct UsbBus {
    bus_id: String,
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    complete_urb: UnboundedSender<Urb>,
    poll_sender: watch::Sender<()>,
    poll_receiver: watch::Receiver<()>,
    ep_in: [Endpoint; NUM_ENDPOINTS],
    ep_out: [Endpoint; NUM_ENDPOINTS],
}

/// Virtual USB peripheral driver
impl UsbBus {
    pub(crate) fn new(bus_id: &str) -> UsbBusAllocator<Pin<Box<Self>>> {
        let (poll_sender, poll_receiver) = mpsc::unbounded_channel();

        UsbBusAllocator::new(Box::pin(UsbBus {
            bus_id: String::from(bus_id),
            urb_queue: Arc::new(Mutex::new(VecDeque::new())),
            complete_urb
            poll_sender,
            poll_receiver,
            device,
            ep_in: Default::default(),
            ep_out: Default::default(),
        }))
    }
}

impl UsbBus {
    pub fn events(&self) -> watch::Receiver<()> {
        self.poll_receiver.clone()
    }

    fn current_urb(&self, ep_addr: EndpointAddress, urb: &mut Option<Urb>) -> Option<&mut Urb> {
        if urb.is_some() {
            return urb.as_mut();
        }

        let mut urb_queue = self.urb_queue.lock().unwrap();

        match urb_queue.iter()
            .enumerate()
            .find(|u| u.ep == ep_addr)
        {
            Some((index, urb)) => {
                urb_queue.remove(index);
                *urb = Some(urb);
                urb.as_mut()
            },
            None => None,
        }
    }

    fn complete_urb(&self, urb: Urb) {
        unimplemented!();
    }
}

impl usb_device::bus::UsbBus for Pin<Box<UsbBus>> {
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

        let mut state = ep.state.lock().unwrap();

        let urb = match self.current_urb(ep_addr, &mut state.urb) {
            // There is an active URB
            Some(urb) => urb,

            // No active URB, try to store packet in the packet buffer
            None => {
                match state.packet {
                    // Buffer is already in use
                    Some(_) => return Err(UsbError::WouldBlock),

                    // Store packet in buffer
                    None => {
                        state.packet = Some(BytesMut::from(buf));
                        return Ok(buf.len());
                    },
                }
            }
        };

        if let Some(packet) = state.packet.take() {
            // There is a packet waiting in the buffer, add it to the URB
            urb.data.extend_from_slice(&packet);

            if urb.data.len() == urb.len || packet.len() < ep.max_packet_size {
                // The buffered packet completed the URB, store the current data in the buffer
                // instead.

                self.complete_urb(urb.take().unwrap());

                state.packet = Some(BytesMut::from(buf));

                return Ok(buf.len());
            }
        }

        // Add the buffer to the URB
        urb.data.extend_from_slice(buf);

        // If more data than the URB requested has been written, store the rest in the packet
        // buffer.
        if urb.data.len() > urb.len {
            self.packet = Some(urb.data.split_off(urb.len));
        }

        if urb.data.len() == urb.len || buf.len() < ep.max_packet_size {
            // The URB is complete
            
            self.complete_urb(urb.take().unwrap());
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

        if buf.len() < ep.max_packet_Size {
            return Err(UsbError::BufferOverflow);
        }
        
        let mut state = ep.state.lock().unwrap();

        let urb = match self.current_urb(ep_addr, &mut state.urb) {
            // There is an active URB
            Some(urb) => urb,

            // There is no URB
            None => return Err(UsbError::WouldBlock),
        };

        if urb.data.len() <= buf.len() {
            // The remaining data will be returned by this read, so the URB will be completed

            let len = urb.data.len();
            buf.copy_from_slice(&urb.data);

            self.complete_urb();

            Ok(len)
        } else {
            // A single packet will be read

            let len = ep.max_packet_size;
            buf.copy_from_slice(&buf.data.splic_to(len));

            return Ok(len);
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
    pub packet: Option<BytesMut>,
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