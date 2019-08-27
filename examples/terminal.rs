//! A simple CDC-ACM "loopback in uppercase" serial port example, using USB/IP.

use std::sync::Arc;
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use usbip_usbd::{UsbIpDeviceBuilder, Server};
use usbd_serial::{USB_CLASS_CDC, SerialPort};
use tokio::prelude::*;
use tokio::sync::lock::Lock;

#[tokio::main]
fn main() {
    let listener = Server::bind(&"127.0.0.1:3240".parse().unwrap())
        .expect("Failed to create server");

    loop {
        let connection = listener.accept();

        let usb_bus = connection.attach("1-1");

        let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")
            .device_class(USB_CLASS_CDC)
            .build();

        let serial_rx = Arc::new(Lock::new(SerialPort::new(usb_bus)));

        let event = usb_dev.bus().event();
        tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();

            loop {
                if !usb_dev.poll(&mut [&mut *serial_rx.lock().await]) {
                    event.await?;
                    continue;
                }

                match serial_rx.lock().await.read(&mut buf[..]) {
                    Ok(count) => {
                        stdout.write_buf(&buf[..count]).await.unwrap();
                    },
                    Err(UsbError::WouldBlock) => event.await?,
                    Err(err) => {
                        println!("Read error: {:?}", err);
                    }
                }
            }
        });
    }
}
