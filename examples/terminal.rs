//! Output data written on a virtual serial port on stdout.

use usb_device::prelude::*;
use usbip_usbd::Server;
use usbd_serial::{USB_CLASS_CDC, SerialPort};
use tokio::io::AsyncWriteExt as _;
use tokio::time::delay_for;
use std::process::Command;
use std::time::Duration;
//use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let mut listener = Server::bind("127.0.0.1:3240").await
        .expect("Failed to create server");

    let ip = listener.local_addr().unwrap().ip();

    println!("USB-IP server is running.");
    println!("Try:");
    println!("  [modprobe vhci-hcd]");
    println!("  usbip list -r {}", ip);
    println!("  usbip attach -r {} -b 1-1", ip);

    Command::new("usbip")
        .arg("-d")
        //.arg("list")
        .arg("attach")
        .arg("-r").arg(ip.to_string())
        .arg("-b").arg("1-1")
        .spawn()
        .expect("failed to spawn usbip");

    while let Ok(mut client) = listener.accept().await {
        tokio::spawn(async move {
            let mut serial = SerialPort::new();

            let (usbcore, mut _poller) = client.attach("1-1");

            let mut usb_dev = UsbDeviceBuilder::new(usbcore, UsbVidPid(0x16c0, 0x27dd))
                .manufacturer("Fake company")
                .product("USB-IP port")
                .serial_number("TEST")
                .device_class(USB_CLASS_CDC)
                .build(&mut serial)
                .expect("Building device failed");

            let mut stdout = tokio::io::stdout();

            tokio::spawn(client.run());

            loop {
                delay_for(Duration::from_millis(10)).await;

                // TODO: figure out when we need to fire this
                //poller.poll().await;

                //println!("pollo");

                if usb_dev.poll(&mut serial).is_err() {
                    continue;
                }

                let mut buf = [0u8; 1024];

                loop {
                    match serial.read(&mut buf[..]) {
                        Ok(count) => {
                            stdout.write_all(&buf[..count]).await.expect("failed to write to stdout");
                            stdout.flush().await.expect("failed to flush stdout");
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
    }
}
