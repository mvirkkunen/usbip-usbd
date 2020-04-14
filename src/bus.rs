use tokio::sync::watch;
use usb_device::{
    Result, UsbError, UsbDirection,
    allocator::{EndpointConfig, UsbAllocator},
    //class::UsbClass,
    endpoint::{EndpointDescriptor, EndpointAddress},
    bus::PollResult,
};
use crate::server::BusChannel;
use crate::endpoint::{EndpointOut, EndpointIn};

pub const NUM_ENDPOINTS: usize = 16;

pub struct UsbBus {
    channel: BusChannel,
}

/// Virtual USB peripheral driver
impl UsbBus {
    pub(crate) fn new(channel: BusChannel) -> UsbAllocator<Self> {
        UsbAllocator::new(UsbBus {
            channel,
        })
    }

    /*pub fn register_class<C: UsbClass<Self>>(cls: &Arc<Lock<C>>) {

    }*/
}

impl UsbBus {
    pub fn poller(&self) -> watch::Receiver<()> {
        self.channel.poller()
    }
}

impl usb_device::bus::UsbBus for UsbBus {
    type EndpointOut = EndpointOut;
    type EndpointIn = EndpointIn;
    type EndpointAllocator = EndpointAllocator;

    fn create_allocator(&mut self) -> EndpointAllocator {
        EndpointAllocator::new(self.channel.clone())
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
        // TODO
        
        PollResult::Data {
            ep_out: 0xffff,
            ep_in_complete: 0xffff,
            ep_setup: 0x001,
        }
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
