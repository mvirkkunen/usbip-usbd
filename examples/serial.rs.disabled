//! A simple CDC-ACM "loopback in uppercase" serial port example, using USB/IP.

use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use usbip_usbd::{UsbBus, Server};
use usbd_serial::{USB_CLASS_CDC, SerialPort};
use tokio::prelude::*;
use tokio::sync::lock::Lock;
use std::sync::Arc;

fn main() {
    let listener = Server::bind(&"127.0.0.1:3240".parse().unwrap()).unwrap();

    let usb_bus = unsafe {
        static mut USB_BUS: Option<UsbBusAllocator<UsbBus>> = None;
        USB_BUS = Some(listener.attach("1-1"));
        USB_BUS.as_ref().unwrap()
    };

    let mut serial = Arc::new(Lock::new(SerialPort::new(usb_bus)));

    let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    let mut buf = [0u8; 64];
    let mut pos: usize = 0;
    let mut len: usize = 0;

    usb_dev.bus().register_class(Arc::clone(&serial));

    let server = usb_dev.bus().events()
        .map(|_| serial.poll_lock())
        .for_each(move |serial| {
            if usb_dev.state() != UsbDeviceState::Configured {
                return Ok(());
            }

            if len == 0 {
                match serial.read(&mut buf[..]) {
                    Ok(count) => {
                        //println!("r {}", count);
                        //println!("read {} {:?}", count, &buf[..count]);

                        for c in buf[..count].iter_mut() {
                            if 0x61 <= *c && *c <= 0x7a {
                                *c &= !0x20;
                            }
                        }

                        pos = 0;
                        len = count;
                    },
                    Err(UsbError::WouldBlock) => { },
                    Err(err) => {
                        println!("Read error: {:?}", err);
                    }
                };
            } else {
                let nwrite = (if len != 64 && len > 10 { len / 2 } else { len });

                match serial.write(&buf[pos..pos+nwrite]) {
                    Ok(count) => {
                        //println!("w {}", count);
                        //println!("write {} {:?}", count, &buf[pos..pos+nwrite]);

                        pos += count;
                        len -= count;
                    },
                    Err(UsbError::WouldBlock) => { },
                    Err(err) => {
                        println!("Write error: {:?}", err);
                    }
                }
            }

            Ok(())
        });

    tokio::run(server);
}
