//! CDC-ACM terminal via virtual serial port

use std::sync::{Arc, Mutex, Condvar};
use usb_device::prelude::*;
use usb_device::bus::UsbBusAllocator;
use usbip_usbd::{UsbBus, Server};
use usbd_serial::{USB_CLASS_CDC, SerialPort};

fn main() {
    let server = Server::bind(&"127.0.0.1:3240".parse().unwrap()).unwrap();

    let (waiter, usb_bus) = server.attach("1-1");

    let usb_bus = unsafe {
        static mut USB_BUS: Option<UsbBusAllocator<UsbBus>> = None;
        USB_BUS = Some(usb_bus);
        USB_BUS.as_ref().unwrap()
    };

    let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial terminal")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    let serial_lock = Arc::new(Mutex::new(SerialPort::new(usb_bus)));
    
    let shared = (Arc::clone(waiter), Arc::clone(serial_mutex));
    let reader = thread::spawn(move || {
        let mut (waiter, serial_lock) = shared;
        let mut stdout = std::io::stdout();

        loop {
            let mut serial = serial_lock.lock().unwrap();

            if !usb_dev.poll(&mut [&mut serial]) {
                continue;
            }

            let buf = [0u8; 256];

            match serial.read(&mut buf[..]) {
                Ok(count) => {
                    stdout.write_all(&buf[..count]).unwrap();
                },
                Err(UsbError::WouldBlock) => { },
                Err(err) => {
                    println!("Read error: {:?}", err);
                }
            }

            waiter.wait()?;
        }
    });

    let writer = thread::spawn(move || {
        let mut stdin = std::io::stdin();

        loop {
            let mut buf = [0u8; 256];
            let len = stdio.read(&mut buf[..]).unwrap();
            let pos = 0;

            while pos < len {
                let result = { serial_lock.lock().unwrap().write(&buf[pos..len]) };

                match result {
                    Ok(count) => { pos += count; },
                    Err(UsbError::WouldBlock) => waiter.wait()?,
                    Err(err) => println!("Write error: {:?}", err),
                }
            }
        }
    });

    reader.join();
    writer.join();
}
