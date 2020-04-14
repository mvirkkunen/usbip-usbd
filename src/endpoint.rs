use usb_device::{
    Result, UsbDirection, UsbError,
    usbcore,
    endpoint::{EndpointAddress, EndpointConfig, OutPacketType},
};
use crate::server::{ControlState, CoreChannel, Urb};

fn update_urb<'a>(
    ep_addr: EndpointAddress,
    urb: &'a mut Option<Urb>,
    channel: &mut CoreChannel) -> Option<&'a mut Urb>
{
    if urb.is_none() {
        *urb = channel.take_next_urb(ep_addr);
    }

    urb.as_mut()
}

pub struct EndpointOut {
    address: EndpointAddress,
    max_packet_size: usize,
    channel: CoreChannel,
    stalled: bool,
    urb: Option<Urb>,
}

impl EndpointOut {
    pub fn new(address: EndpointAddress, max_packet_size: usize, channel: CoreChannel) -> EndpointOut {
        EndpointOut {
            address,
            max_packet_size,
            channel,
            stalled: false,
            urb: None,
        }
    }
}

impl usbcore::UsbEndpoint for EndpointOut {
    fn address(&self) -> EndpointAddress {
        self.address
    }

    unsafe fn enable(&mut self, _config: &EndpointConfig) -> Result<()> {
        // TODO
        Ok(())
    }

    fn disable(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }

    fn set_stalled(&mut self, is_stalled: bool) -> Result<()> {
        self.stalled = is_stalled;

        Ok(())
    }

    fn is_stalled(&mut self) -> Result<bool> {
        Ok(self.stalled)
    }
}

impl usbcore::UsbEndpointOut for EndpointOut {
    fn read_packet(&mut self, buf: &mut [u8]) -> Result<(usize, OutPacketType)> {
        println!("read {:?}", self.address);

        if buf.len() < self.max_packet_size {
            return Err(UsbError::BufferOverflow);
        }

        let urb = update_urb(self.address, &mut self.urb, &mut self.channel)
            .ok_or(UsbError::WouldBlock)?;

        if let Some(ref mut control) = urb.control {
            match control.state {
                ControlState::Setup => {
                    // Return SETUP data

                    let req = &control.request;

                    buf[..8].copy_from_slice(&[
                        // bmRequestType
                        (req.direction as u8) | ((req.request_type as u8) << 5) | (req.recipient as u8),
                        // bRequest
                        req.request,
                        // wValue
                        req.value as u8, (req.value >> 8) as u8,
                        // wIndex
                        req.index as u8, (req.index >> 8) as u8,
                        // wLength
                        req.length as u8, (req.length >> 8) as u8,
                    ]);

                    if urb.len > 0 {
                        // There is a data stage

                        control.state = ControlState::Data;

                        if req.direction == UsbDirection::In {
                            // Data is in the other direction - pass to other endpoint

                            self.channel.complete_urb(self.urb.take().unwrap());
                        }
                    } else {
                        control.state = ControlState::Status;

                        // No data stage - pass to other endpoint for status

                        self.channel.complete_urb(self.urb.take().unwrap());
                    }

                    return Ok((8, OutPacketType::Setup));
                },

                ControlState::Data => { /* handled below */ },

                ControlState::Status => {
                    // Return empty STATUS packet

                    control.state = ControlState::Complete;

                    self.channel.complete_urb(self.urb.take().unwrap());

                    return Ok((0, OutPacketType::Data));
                },

                ControlState::Complete => panic!("Complete control USB passed to OUT endpoint"),
            }
        }

        if urb.data.len() <= self.max_packet_size {
            // The remaining data will be returned by this read, so the URB will be completed

            // TODO: Do we need to simulate ZLP

            let len = urb.data.len();
            buf[..len].copy_from_slice(&urb.data);

            if let Some(ref mut control) = urb.control {
                match control.state {
                    ControlState::Setup => panic!("Invalid read in Setup state"),

                    ControlState::Data => {
                        control.state = ControlState::Status;
                    },

                    ControlState::Status => panic!("Invalid read in Status state"),

                    ControlState::Complete => panic!("Complete control USB passed to OUT endpoint"),
                }
            }

            self.channel.complete_urb(self.urb.take().unwrap());

            Ok((len, OutPacketType::Data))
        } else {
            // A single packet will be read

            let len = self.max_packet_size;
            buf.copy_from_slice(urb.data.split_to(len).as_ref());

            Ok((len, OutPacketType::Data))
        }
    }
}

pub struct EndpointIn {
    address: EndpointAddress,
    max_packet_size: usize,
    channel: CoreChannel,
    stalled: bool,
    urb: Option<Urb>,
}

impl EndpointIn {
    pub fn new(address: EndpointAddress, max_packet_size: usize, channel: CoreChannel) -> EndpointIn {
        EndpointIn {
            address,
            max_packet_size,
            channel,
            stalled: false,
            urb: None,
        }
    }
}

impl usbcore::UsbEndpoint for EndpointIn {
    fn address(&self) -> EndpointAddress {
        self.address
    }

    unsafe fn enable(&mut self, _config: &EndpointConfig) -> Result<()> {
        // TODO
        Ok(())
    }

    fn disable(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }

    fn set_stalled(&mut self, is_stalled: bool) -> Result<()> {
        self.stalled = is_stalled;

        Ok(())
    }

    fn is_stalled(&mut self) -> Result<bool> {
        Ok(self.stalled)
    }
}

impl usbcore::UsbEndpointIn for EndpointIn {
    fn write_packet(&mut self, buf: &[u8]) -> Result<()> {
        if buf.len() > self.max_packet_size {
            return Err(UsbError::BufferOverflow);
        }

        println!("writing {:?}", buf);

        let urb = update_urb(self.address, &mut self.urb, &mut self.channel)
            .ok_or(UsbError::WouldBlock)?;

        // Add the buffer to the URB
        urb.data.extend_from_slice(buf);

        if buf.len() < self.max_packet_size {
            // The URB is complete

            if let Some(ref mut control) = urb.control {
                match control.state {
                    ControlState::Setup => panic!("SETUP passed to IN endpoint"),

                    ControlState::Data => {
                        control.state = ControlState::Status;
                    }

                    ControlState::Status => {
                        control.state = ControlState::Complete;
                    },

                    ControlState::Complete => panic!("Complete control USB passed to IN endpoint"),
                }
            }

            self.channel.complete_urb(self.urb.take().unwrap());
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        // TODO
        Ok(())
    }
}
