//! A simple CDC-ACM "loopback in uppercase" serial port example, using USB/IP.

use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use usbip_usbd::{UsbBus, Server};
use usbd_serial::{USB_CLASS_CDC, SerialPort};
use tokio::prelude::*;
use tokio::sync::lock::Lock;

fn main() {
    static mut USB_BUS: Option<UsbBusAllocator<UsbBus>> = None;

    let listener = Server::bind(&"127.0.0.1:3240".parse().unwrap()).unwrap();

    let (stream, usb_bus) = listener.attach("1-1");

    let usb_bus = unsafe {
        USB_BUS = Some(usb_bus);
        USB_BUS.as_ref().unwrap()
    };

    let mut serial = SerialPort::new(usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    let mut stdout = tokio::io::stdout();

    let usb_stream = stream.for_each(move |_| {
        if !usb_dev.poll(&mut [&mut serial]) && len == 0 {
            return Ok(());
        }

        let buf = [0u8; 64];

        match serial.read(&mut buf[..]) {
            Ok(count) => {
                stdout.write_buf(&buf[..count])
            },
            Err(UsbError::WouldBlock) => { Ok(()) },
            Err(err) => {
                println!("Read error: {:?}", err);
                Ok(())
            }
        }
    });

    let stdout_stream = tokio::io::lines(tokio::io::stdin())
        .for_each(|lines| {
            
        });


    tokio::run(server);
}
