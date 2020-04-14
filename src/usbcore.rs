use tokio::sync::watch;
use usb_device::{
    Result, UsbError, UsbDirection,
    //class::UsbClass,
    endpoint::{EndpointAddress, EndpointConfig},
    usbcore::{self, PollResult},
};
use crate::server::CoreChannel;
use crate::endpoint::{EndpointOut, EndpointIn};

pub const NUM_ENDPOINTS: usize = 16;

pub struct UsbCore {
    channel: CoreChannel,
}

/// Virtual USB peripheral driver
impl UsbCore {
    pub(crate) fn new(channel: CoreChannel) -> UsbCore {
        UsbCore { channel }
    }
}

impl usbcore::UsbCore for UsbCore {
    type EndpointOut = EndpointOut;

    type EndpointIn = EndpointIn;

    type EndpointAllocator = EndpointAllocator;

    fn create_allocator(&mut self) -> EndpointAllocator {
        EndpointAllocator::new(self.channel.clone())
    }

    fn enable(&mut self, _alloc: EndpointAllocator) -> Result<()> {
        // nop
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }

    fn set_device_address(&mut self, _addr: u8) -> Result<()> {
        // nop
        Ok(())
    }

    fn poll(&mut self) -> Result<PollResult> {
        // TODO

        Ok(PollResult::Data {
            ep_out: 0xffff,
            ep_in_complete: 0xffff,
        })
    }

    fn set_stalled(&mut self, ep_addr: EndpointAddress, stalled: bool) -> Result<()> {
        let _ = ep_addr;
        let _ = stalled;
        unimplemented!();
    }

    fn is_stalled(&mut self, ep_addr: EndpointAddress) -> Result<bool> {
        let _ = ep_addr;
        unimplemented!();
    }

    fn suspend(&mut self) -> Result<()> {
        // nop
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        // nop
        Ok(())
    }
}

pub struct EndpointAllocator {
    channel: CoreChannel,
    next_endpoint_number: u8,
    out_taken: u16,
    in_taken: u16,
}

impl EndpointAllocator {
    fn new(channel: CoreChannel) -> Self {
        EndpointAllocator {
            channel,
            next_endpoint_number: 1,
            out_taken: 0,
            in_taken: 0,
        }
    }

    fn alloc_ep(&mut self, direction: UsbDirection, config: &EndpointConfig)
        -> Result<(EndpointAddress, usize)>
    {
        let number = config.fixed_address()
            .map(|a| a.number())
            .unwrap_or(self.next_endpoint_number);

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

        /*let descriptor = EndpointDescriptor {
            address: EndpointAddress::from_parts(number, direction),
            ep_type: config.ep_type,
            max_packet_size: config.max_packet_size,
            interval: config.interval,
        };*/

        if config.fixed_address().is_none() {
            self.next_endpoint_number += 1;
        }

        Ok((EndpointAddress::from_parts(number, direction), config.max_packet_size().into()))
    }
}

impl usb_device::usbcore::UsbEndpointAllocator<UsbCore> for EndpointAllocator {
    fn alloc_out(&mut self, config: &EndpointConfig) -> Result<EndpointOut> {
        let (address, max_packet_size) = self.alloc_ep(UsbDirection::Out, config)?;

        Ok(EndpointOut::new(address, max_packet_size, self.channel.clone()))
    }

    fn alloc_in(&mut self, config: &EndpointConfig) -> Result<EndpointIn> {
        let (address, max_packet_size) = self.alloc_ep(UsbDirection::In, config)?;

        Ok(EndpointIn::new(address, max_packet_size, self.channel.clone()))
    }

    fn begin_interface(&mut self) -> Result<()> {
        Ok(())
    }

    fn next_alt_setting(&mut self) -> Result<()> {
        Ok(())
    }
}
