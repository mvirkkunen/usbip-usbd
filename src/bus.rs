use std::sync::Arc;
use usb_device::{
    Result, UsbError, UsbDirection,
    endpoint::{EndpointType, EndpointAddress},
    bus::{UsbBusAllocator, PollResult},
};
use crate::device::Device;

pub const NUM_ENDPOINTS: usize = 16;

pub struct UsbBus {
    device: Arc<Device>,
}

impl UsbBus {
    pub(crate) fn new(device: Arc<Device>) -> UsbBusAllocator<UsbBus> {
        UsbBusAllocator::new(UsbBus {
            device
        })
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

    }

    fn set_device_address(&self, _addr: u8) {
        // nop
    }

    fn poll(&self) -> PollResult {
        unimplemented!();
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize> {
        unimplemented!();
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize> {
        unimplemented!();
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