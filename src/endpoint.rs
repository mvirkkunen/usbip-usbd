use bytes::Bytes;
use usb_device::{
    Result, UsbError, UsbDirection,
    endpoint::{
        Endpoint as _,
        EndpointAddress,
        EndpointDescriptor
    }
};
use crate::server::{Urb, BusChannel, ControlState};

fn update_urb<'a>(
    ep_addr: EndpointAddress,
    urb: &'a mut Option<Urb>,
    channel: &mut BusChannel) -> &'a mut Option<Urb>
{
    match urb {
        Some(_) => urb,
        None => {
            *urb = channel.take_next_urb(ep_addr);
            urb
        }
    }
}

pub struct EndpointOut {
    descriptor: EndpointDescriptor,
    channel: BusChannel,
    stalled: bool,
    urb: Option<Urb>,
}

impl EndpointOut {
    pub fn new(descriptor: EndpointDescriptor, channel: BusChannel) -> EndpointOut {
        EndpointOut {
            descriptor,
            channel,
            stalled: false,
            urb: None,
        }
    }
}

impl usb_device::endpoint::Endpoint for EndpointOut {
    fn descriptor(&self) -> &EndpointDescriptor { &self.descriptor }

    fn enable(&mut self) {
        // TODO
    }

    fn disable(&mut self) {
        unimplemented!();
    }

    fn set_stalled(&mut self, is_stalled: bool) {
        self.stalled = is_stalled;
    }

    fn is_stalled(&self) -> bool {
        self.stalled
    }
}

impl usb_device::endpoint::EndpointOut for EndpointOut {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() < self.max_packet_size() as usize {
            return Err(UsbError::BufferOverflow);
        }

        let max_packet_size = self.max_packet_size() as usize;

        let urb = match update_urb(self.address(), &mut self.urb, &mut self.channel) {
            // There is an active URB
            Some(urb) => urb,

            // There is no URB
            None => return Err(UsbError::WouldBlock),
        };

        if let Some(mut control) = urb.control {
            match control.state {
                ControlState::Setup => {
                    // This is the SETUP part of a control transfer, return the SETUP packet

                    match urb.req_ep.direction() { 
                        UsbDirection::Out => {
                            // Control OUT - this endpoint will keep the request

                            if control.length > 0 {
                                // DATA stage - prepare to read data
                            }
                        },

                        UsbDirection::In => {

                        },
                    }

                    buf[..8].copy_from_slice(&control.setup);

                    return Ok(8);
                },
                ControlState::DataOut => {
                    // continue handling below like any URB
                }
            }
        }
        
        if urb.data.len() <= buf.len() {
            // The remaining data will be returned by this read, so the URB will be completed

            let len = urb.data.len();
            buf[..len].copy_from_slice(&urb.data);

            self.channel.complete_urb(self.urb.take().unwrap());

            Ok(len)
        } else {
            // A single packet will be read

            let len = max_packet_size;
            buf.copy_from_slice(urb.data.split_to(len).as_ref());

            Ok(len);
        }
    }
}

pub struct EndpointIn {
    descriptor: EndpointDescriptor,
    channel: BusChannel,
    stalled: bool,
    urb: Option<Urb>,
    leftover: Option<Bytes>,
}

impl EndpointIn {
    pub fn new(descriptor: EndpointDescriptor,  channel: BusChannel) -> EndpointIn {
        EndpointIn {
            descriptor,
            channel,
            stalled: false,
            urb: None,
            leftover: None,
        }
    }
}

impl usb_device::endpoint::Endpoint for EndpointIn {
    fn descriptor(&self) -> &EndpointDescriptor { &self.descriptor }
    
    fn enable(&mut self) {
        // TODO
    }

    fn disable(&mut self) {
        unimplemented!();
    }

    fn set_stalled(&mut self, is_stalled: bool) {
        self.stalled = is_stalled;
    }

    fn is_stalled(&self) -> bool {
        self.stalled
    }
}

impl usb_device::endpoint::EndpointIn for EndpointIn {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        if buf.len() > self.max_packet_size() as usize {
            return Err(UsbError::BufferOverflow);
        }

        println!("writing {:?}", buf);

        let max_packet_size = self.max_packet_size() as usize;

        let urb = match update_urb(self.address(), &mut self.urb, &mut self.channel) {
            // There is an active URB
            Some(urb) => urb,

            // No active URB, try to store packet in the leftover buffer
            None => return match self.leftover {
                // Leftover buffer is already in use
                Some(_) => Err(UsbError::WouldBlock),

                // Store packet in leftover buffer
                None => {
                    self.leftover = Some(Bytes::from(buf));
                    Ok(())
                },
            },
        };

        if let Some(leftover) = self.leftover.take() {
            // There is a packet waiting in the buffer, add it to the URB
            urb.data.extend_from_slice(&leftover);

            if urb.data.len() >= urb.len || leftover.len() < max_packet_size {
                // The leftover buffer completed the URB

                if urb.data.len() > urb.len {
                    self.leftover = Some(urb.data.split_off(urb.len).freeze());
                }

                self.channel.complete_urb(self.urb.take().unwrap());

                return if self.leftover.is_none() {
                    // Store the current packet in the leftover buffer instead
                    self.leftover = Some(Bytes::from(buf));

                    Ok(())
                } else {
                    // There is still data in the leftover buffer

                    Err(UsbError::WouldBlock)
                };
            }
        }

        // Add the buffer to the URB
        urb.data.extend_from_slice(buf);

        // If more data than the URB requested has been written, store the rest in the packet
        // buffer.
        if urb.data.len() > urb.len {
            self.leftover = Some(urb.data.split_off(urb.len).freeze());
        }

        if urb.data.len() == urb.len || buf.len() < self.max_packet_size() as usize {
            // The URB is complete
            
            self.channel.complete_urb(self.urb.take().unwrap());
        }

        Ok(())
    }
}
