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

    let mut buf = [0u8; 64];
    let mut pos: usize = 0;
    let mut len: usize = 0;

    let server = stream.for_each(move |_| {
        if !usb_dev.poll(&mut [&mut serial]) && len == 0 {
            return Ok(());
        }

        if len == 0 {
            match serial.read(&mut buf[..]) {
                Ok(count) => {
                    println!("r {}", count);
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

            //println!("writing {}", nwrite);

            match serial.write(&buf[pos..pos+nwrite]) {
                Ok(count) => {
                    println!("w {}", count);
                    //println!("write {} {:?}", count, &buf[pos..pos+nwrite]);

                    //if count == 0 {
                // }

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
