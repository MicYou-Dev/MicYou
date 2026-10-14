/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

//! USB connections to the MicYou iOS app through usbmuxd.
//!
//! usbmuxd ships with macOS, comes with iTunes on Windows and is the
//! `usbmuxd` package on Linux. It only lets the computer open connections to the phone, so in USB mode
//! the iOS app listens on its loopback interface and [`run_bridge`] connects
//! to it, then splices the stream into the local TCP server. To the server
//! the phone looks like any other USB client arriving on loopback.

use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;

const HEADER_LEN: usize = 16;
const VERSION_PLIST: u32 = 1;
const MESSAGE_PLIST: u32 = 8;
/// usbmuxd replies are small; anything larger is a broken or hostile peer.
const MAX_REPLY_LEN: usize = 1024 * 1024;
const RESULT_OK: u64 = 0;
const RESULT_CONNECTION_REFUSED: u64 = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// An iOS device attached over USB. The id is usbmuxd's handle for this
/// attachment; the serial number (UDID) is deliberately not kept, so it
/// cannot end up in logs people share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbDevice {
    pub id: u64,
}

#[derive(Debug)]
pub enum ConnectError {
    /// Nothing listens on that port: the app is closed or not in USB mode.
    Refused,
    Io(io::Error),
}

impl From<io::Error> for ConnectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

enum Socket {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    Tcp(std::net::TcpStream),
}

impl Socket {
    /// `USBMUXD_SOCKET_ADDRESS` is the libusbmuxd override, `host:port` for
    /// TCP or a socket path; otherwise the platform default.
    fn open() -> io::Result<Self> {
        let socket = match std::env::var("USBMUXD_SOCKET_ADDRESS") {
            #[cfg(unix)]
            Ok(address) if !address.contains(':') => {
                Self::Unix(std::os::unix::net::UnixStream::connect(address)?)
            }
            Ok(address) => Self::Tcp(std::net::TcpStream::connect(address)?),
            #[cfg(unix)]
            Err(_) => Self::Unix(std::os::unix::net::UnixStream::connect("/var/run/usbmuxd")?),
            #[cfg(not(unix))]
            Err(_) => Self::Tcp(std::net::TcpStream::connect(("127.0.0.1", 27015))?),
        };
        socket.set_timeout(Some(REQUEST_TIMEOUT))?;
        Ok(socket)
    }

    fn set_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => {
                s.set_read_timeout(timeout)?;
                s.set_write_timeout(timeout)
            }
            Self::Tcp(s) => {
                s.set_read_timeout(timeout)?;
                s.set_write_timeout(timeout)
            }
        }
    }

    fn into_async(self) -> io::Result<Box<dyn AsyncStream>> {
        self.set_timeout(None)?;
        Ok(match self {
            #[cfg(unix)]
            Self::Unix(s) => {
                s.set_nonblocking(true)?;
                Box::new(tokio::net::UnixStream::from_std(s)?)
            }
            Self::Tcp(s) => {
                s.set_nonblocking(true)?;
                s.set_nodelay(true)?;
                Box::new(tokio::net::TcpStream::from_std(s)?)
            }
        })
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            Self::Tcp(s) => s.read(buf),
        }
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
            Self::Tcp(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
            Self::Tcp(s) => s.flush(),
        }
    }
}

pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Frames a plist request: length (header included), version, message type and tag, all little endian.
fn encode_request(message_type: &str, extra: plist::Dictionary, tag: u32) -> io::Result<Vec<u8>> {
    let mut dict = plist::Dictionary::new();
    dict.insert("MessageType".into(), message_type.into());
    dict.insert("ProgName".into(), "MicYou".into());
    dict.insert("ClientVersionString".into(), "micyou-usbmux".into());
    dict.insert("kLibUSBMuxVersion".into(), 3u64.into());
    for (key, value) in extra {
        dict.insert(key, value);
    }
    let mut body = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut body)
        .map_err(|e| invalid(e.to_string()))?;
    let total = u32::try_from(HEADER_LEN + body.len()).map_err(|_| invalid("request too large"))?;
    let mut frame = Vec::with_capacity(HEADER_LEN + body.len());
    for word in [total, VERSION_PLIST, MESSAGE_PLIST, tag] {
        frame.extend_from_slice(&word.to_le_bytes());
    }
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Body length of a reply from its header, rejecting malformed sizes and non-plist messages.
fn reply_body_len(header: &[u8; HEADER_LEN]) -> io::Result<usize> {
    let word = |i: usize| u32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
    let total = word(0) as usize;
    if !(HEADER_LEN..=MAX_REPLY_LEN).contains(&total) {
        return Err(invalid(format!("bad usbmuxd reply length {total}")));
    }
    if word(2) != MESSAGE_PLIST {
        return Err(invalid(format!("unexpected usbmuxd message type {}", word(2))));
    }
    Ok(total - HEADER_LEN)
}

fn request(socket: &mut Socket, message_type: &str, extra: plist::Dictionary) -> io::Result<plist::Dictionary> {
    socket.write_all(&encode_request(message_type, extra, 1)?)?;
    let mut header = [0u8; HEADER_LEN];
    socket.read_exact(&mut header)?;
    let mut body = vec![0u8; reply_body_len(&header)?];
    socket.read_exact(&mut body)?;
    plist::from_bytes::<plist::Value>(&body)
        .map_err(|e| invalid(e.to_string()))?
        .into_dictionary()
        .ok_or_else(|| invalid("usbmuxd reply is not a dictionary"))
}

/// Devices attached over USB; devices paired for Wi-Fi sync are left out.
fn parse_device_list(reply: &plist::Dictionary) -> io::Result<Vec<UsbDevice>> {
    let list = reply
        .get("DeviceList")
        .and_then(plist::Value::as_array)
        .ok_or_else(|| invalid("usbmuxd reply has no DeviceList"))?;
    Ok(list
        .iter()
        .filter_map(plist::Value::as_dictionary)
        .filter_map(|entry| entry.get("Properties")?.as_dictionary())
        .filter(|props| props.get("ConnectionType").and_then(plist::Value::as_string) == Some("USB"))
        .filter_map(|props| {
            Some(UsbDevice {
                id: props.get("DeviceID")?.as_unsigned_integer()?,
            })
        })
        .collect())
}

/// Lists iOS devices attached over USB. Fails when usbmuxd is not running.
pub fn list_usb_devices() -> io::Result<Vec<UsbDevice>> {
    let mut socket = Socket::open()?;
    parse_device_list(&request(&mut socket, "ListDevices", plist::Dictionary::new())?)
}

/// True when usbmuxd reports at least one iOS device over USB.
pub fn has_usb_device() -> bool {
    list_usb_devices().is_ok_and(|devices| !devices.is_empty())
}

/// Opens a connection to `port` on the device; the socket becomes a plain
/// byte stream to the app once usbmuxd accepts.
fn connect(device_id: u64, port: u16) -> Result<Socket, ConnectError> {
    let mut socket = Socket::open()?;
    let mut extra = plist::Dictionary::new();
    extra.insert("DeviceID".into(), device_id.into());
    // usbmuxd expects the port in network byte order.
    extra.insert("PortNumber".into(), u64::from(port.to_be()).into());
    let reply = request(&mut socket, "Connect", extra)?;
    match reply.get("Number").and_then(plist::Value::as_unsigned_integer) {
        Some(RESULT_OK) => Ok(socket),
        Some(RESULT_CONNECTION_REFUSED) => Err(ConnectError::Refused),
        other => Err(ConnectError::Io(invalid(format!("usbmuxd Connect failed: {other:?}")))),
    }
}

/// Keeps every attached iOS device that runs MicYou in USB mode bridged to
/// the local server on `port`, until `cancel` fires.
pub async fn run_bridge(port: u16, cancel: CancellationToken) {
    let bridged: Arc<Mutex<HashSet<u64>>> = Arc::default();
    let mut usbmuxd_missing_logged = false;
    loop {
        match tokio::task::spawn_blocking(list_usb_devices).await {
            Ok(Ok(devices)) => {
                usbmuxd_missing_logged = false;
                for device in devices {
                    let fresh = bridged
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(device.id);
                    if fresh {
                        tokio::spawn(bridge_device(device.id, port, bridged.clone(), cancel.clone()));
                    }
                }
            }
            Ok(Err(e)) if !usbmuxd_missing_logged => {
                log::info!("[USB] usbmuxd unavailable, iOS over USB disabled until it starts: {e}");
                usbmuxd_missing_logged = true;
            }
            _ => {}
        }
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}

async fn bridge_device(
    id: u64,
    port: u16,
    bridged: Arc<Mutex<HashSet<u64>>>,
    cancel: CancellationToken,
) {
    match tokio::task::spawn_blocking(move || connect(id, port)).await {
        Ok(Ok(phone)) => {
            log::info!("[USB] iOS device {id} connected over usbmux");
            if let Err(e) = splice(phone, port, &cancel).await {
                log::info!("[USB] iOS device {id} bridge ended: {e}");
            }
        }
        // The app is not waiting for a USB connection right now.
        Ok(Err(ConnectError::Refused)) => {}
        Ok(Err(ConnectError::Io(e))) => log::debug!("[USB] usbmux connect failed: {e}"),
        Err(e) => log::debug!("[USB] usbmux connect task failed: {e}"),
    }
    bridged.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
}

async fn splice(phone: Socket, port: u16, cancel: &CancellationToken) -> io::Result<()> {
    let mut phone = phone.into_async()?;
    let mut server = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    server.set_nodelay(true)?;
    tokio::select! {
        _ = cancel.cancelled() => Ok(()),
        result = tokio::io::copy_bidirectional(&mut phone, &mut server) => result.map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_header_is_little_endian_and_counts_itself() {
        let frame = encode_request("ListDevices", plist::Dictionary::new(), 7).unwrap();
        let word = |i: usize| u32::from_le_bytes(frame[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(word(0) as usize, frame.len());
        assert_eq!(word(1), VERSION_PLIST);
        assert_eq!(word(2), MESSAGE_PLIST);
        assert_eq!(word(3), 7);
        let body = plist::from_bytes::<plist::Value>(&frame[HEADER_LEN..]).unwrap();
        let body = body.as_dictionary().unwrap();
        assert_eq!(body.get("MessageType").unwrap().as_string(), Some("ListDevices"));
    }

    #[test]
    fn reply_header_rejects_bad_sizes_and_types() {
        let header = |total: u32, message: u32| {
            let mut h = [0u8; HEADER_LEN];
            h[..4].copy_from_slice(&total.to_le_bytes());
            h[8..12].copy_from_slice(&message.to_le_bytes());
            h
        };
        assert_eq!(reply_body_len(&header(116, MESSAGE_PLIST)).unwrap(), 100);
        assert!(reply_body_len(&header(8, MESSAGE_PLIST)).is_err());
        assert!(reply_body_len(&header(u32::MAX, MESSAGE_PLIST)).is_err());
        assert!(reply_body_len(&header(116, 1)).is_err());
    }

    #[test]
    fn device_list_keeps_usb_devices_only() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>DeviceList</key><array>
<dict><key>DeviceID</key><integer>3</integer><key>MessageType</key><string>Attached</string>
<key>Properties</key><dict><key>ConnectionType</key><string>USB</string><key>DeviceID</key><integer>3</integer>
<key>SerialNumber</key><string>00008110-000A</string></dict></dict>
<dict><key>DeviceID</key><integer>9</integer><key>MessageType</key><string>Attached</string>
<key>Properties</key><dict><key>ConnectionType</key><string>Network</string><key>DeviceID</key><integer>9</integer>
<key>SerialNumber</key><string>00008110-000B</string></dict></dict>
</array></dict></plist>"#;
        let reply = plist::from_bytes::<plist::Value>(xml.as_bytes())
            .unwrap()
            .into_dictionary()
            .unwrap();
        assert_eq!(
            parse_device_list(&reply).unwrap(),
            vec![UsbDevice { id: 3 }]
        );
    }

    #[test]
    #[cfg(target_endian = "little")]
    fn port_goes_out_in_network_byte_order() {
        // 8554 = 0x216A; libusbmuxd sends htons(port), which reads as 0x6A21 here
        assert_eq!(8554u16.to_be(), 0x6A21);
    }
}
