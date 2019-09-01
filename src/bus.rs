use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use bytes::BytesMut;
use tokio::prelude::*;
use tokio::sync::mpsc;
use tokio::sync::watch;
use usb_device::{
    Result, UsbError, UsbDirection,
    allocator::{EndpointConfig, UsbAllocator},
    //class::UsbClass,
    endpoint::{EndpointDescriptor, EndpointAddress},
    bus::PollResult,
};
use crate::endpoint::{EndpointOut, EndpointIn};

pub const NUM_ENDPOINTS: usize = 16;

pub struct UsbBus {
    bus_id: String,
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    completer: mpsc::UnboundedSender<Urb>,
    complete_receiver: mpsc::UnboundedReceiver<Urb>,
    //classes: Vec<ClassWrapper>,
    poll_sender: watch::Sender<()>,
    poll_receiver: watch::Receiver<()>,
}

/// Virtual USB peripheral driver
impl UsbBus {
    pub(crate) fn new(bus_id: &str) -> UsbAllocator<Self> {
        let (completer, complete_receiver) = mpsc::unbounded_channel();
        let (poll_sender, poll_receiver) = watch::channel(());

        UsbAllocator::new(UsbBus {
            bus_id: String::from(bus_id),
            urb_queue: Arc::new(Mutex::new(VecDeque::new())),
            complete_receiver,
            completer,
            //classes: Vec::new(),
            poll_sender,
            poll_receiver,
        })
    }

    /*pub fn register_class<C: UsbClass<Self>>(cls: &Arc<Lock<C>>) {

    }*/
}

impl UsbBus {
    pub fn poller(&self) -> watch::Receiver<()> {
        self.poll_receiver.clone()
    }
}

impl usb_device::bus::UsbBus for UsbBus {
    type EndpointOut = EndpointOut;
    type EndpointIn = EndpointIn;
    type EndpointAllocator = EndpointAllocator;

    fn create_allocator(&mut self) -> EndpointAllocator {
        EndpointAllocator::new(BusChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            completer: self.completer.clone(),
        })
    }

    fn enable(&mut self) {
        // nop
    }

    fn reset(&mut self) {
        // TODO
    }

    fn set_device_address(&mut self, _addr: u8) {
        // nop
    }

    fn poll(&mut self) -> PollResult {
        PollResult::None
    }

    fn set_stalled(&mut self, ep_addr: EndpointAddress, stalled: bool) {
        let _ = ep_addr;
        let _ = stalled;
        unimplemented!();
    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        let _ = ep_addr;
        unimplemented!();
    }

    fn suspend(&mut self) {

    }

    fn resume(&mut self) {

    }
}

pub struct EndpointAllocator {
    channel: BusChannel,
    next_endpoint_number: u8,
    out_taken: u16,
    in_taken: u16,
}

impl EndpointAllocator {
    fn new(channel: BusChannel) -> Self {
        EndpointAllocator {
            channel,
            next_endpoint_number: 1,
            out_taken: 0,
            in_taken: 0,
        }
    }

    fn alloc_ep(&mut self, direction: UsbDirection, config: &EndpointConfig) 
        -> Result<EndpointDescriptor>
    {
        // TODO: Use pair_of information

        let number = config.number.unwrap_or(self.next_endpoint_number);
        if number as usize >= NUM_ENDPOINTS {
            return Err(UsbError::EndpointOverflow);
        }

        match direction {
            UsbDirection::Out => {
                if self.out_taken & (1 << number) != 0 {
                    return Err(UsbError::InvalidEndpoint);
                }

                self.out_taken |= 1 << number;
            },
            UsbDirection::In => {
                if self.in_taken & (1 << number) != 0 {
                    return Err(UsbError::InvalidEndpoint);
                }

                self.in_taken |= 1 << number;
            },
        };

        let descriptor = EndpointDescriptor {
            address: EndpointAddress::from_parts(number, direction),
            ep_type: config.ep_type,
            max_packet_size: config.max_packet_size,
            interval: config.interval,
        };

        if config.number.is_none() {
            self.next_endpoint_number += 1;
        }

        Ok(descriptor)
    }
}

impl usb_device::bus::EndpointAllocator<UsbBus> for EndpointAllocator {
    fn alloc_out(&mut self, config: &EndpointConfig) -> Result<EndpointOut> {
        let descriptor = self.alloc_ep(UsbDirection::Out, config)?;

        Ok(EndpointOut::new(descriptor, self.channel.clone()))
    }
    
    fn alloc_in(&mut self, config: &EndpointConfig) -> Result<EndpointIn> {
        let descriptor = self.alloc_ep(UsbDirection::In, config)?;

        Ok(EndpointIn::new(descriptor, self.channel.clone()))
    }
}

struct BusChannelCore {
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    completer: mpsc::UnboundedSender<Urb>,
}

impl BusChannelCore {
    pub fn enqueue_urb(&self, urb: Urb) {
        let mut urb_queue = self.urb_queue.lock().unwrap();

        urb_queue.push_back(urb);
    }

    pub fn channel(&self) -> BusChannel {
        BusChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            completer: self.completer.clone(),
        }
    }
}

pub struct BusChannel {
    urb_queue: Arc<Mutex<VecDeque<Urb>>>,
    completer: mpsc::UnboundedSender<Urb>,
}

impl BusChannel {
    pub fn clone(&self) -> BusChannel {
        BusChannel {
            urb_queue: Arc::clone(&self.urb_queue),
            completer: self.completer.clone(),
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

    pub fn complete_urb(&mut self, urb: Urb) {
        self.completer.send(urb);
    }
}

pub struct Urb {
    pub seqnum: u32,
    pub devid: u32,
    pub ep: EndpointAddress,
    pub len: usize,
    pub setup: Option<[u8; 8]>,
    pub data: BytesMut,
}