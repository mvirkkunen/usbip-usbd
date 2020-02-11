//! A simple CDC-ACM "loopback in uppercase" serial port example, using USB/IP.

use usb_device::prelude::*;
use usbip_usbd::Server;
use usbd_serial::{USB_CLASS_CDC, SerialPort};
use tokio::prelude::*;
use tokio::sync::Lock;

#[tokio::main]
async fn main() {
    let mut listener = Server::bind("127.0.0.1:3240")
        .await
        .expect("Failed to create server");
    
    let ip = listener.local_addr().unwrap().ip();

    println!("USB-IP server is running.");
    println!("Try:");
    println!("  usbip list -r {}", ip);
    println!("  usbip attach -r {} -b 1-1", ip);

    while let Ok(mut client) = listener.accept().await {
        let mut usb_bus = client.attach("1-1");

        let mut serial = Lock::new(SerialPort::new(&mut usb_bus));

        let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")
            .device_class(USB_CLASS_CDC)
            .build();
        
        //usb_dev.bus().register_poll(&serial);

        println!("wow");

        let mut poller = usb_dev.bus().poller();
        tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();

            loop {
                poller.recv().await;

                if !usb_dev.poll(&mut [&mut *serial.lock().await]) {
                    continue;
                }

                let mut buf = [0u8; 1024];

                loop {
                    match serial.lock().await.read(&mut buf[..]) {
                        Ok(count) => {
                            stdout.write_all(&buf[..count]).await.unwrap();
                        },
                        Err(UsbError::WouldBlock) => break,
                        Err(err) => {
                            println!("Read error: {:?}", err);
                            break;
                        }
                    }
                }
            }
        });

        tokio::spawn(async move { client.run().await.ok(); });
    }
}
