use redox::Box;
use redox::fs::file::File;
use redox::io::{Read, Write, Seek, SeekFrom};
use redox::mem;
use redox::net::*;
use redox::ptr;
use redox::rand;
use redox::slice;
use redox::str;
use redox::{String, ToString};
use redox::to_num::*;
use redox::Vec;

#[derive(Copy, Clone)]
#[repr(packed)]
pub struct UDPHeader {
    pub src: n16,
    pub dst: n16,
    pub len: n16,
    pub checksum: Checksum,
}

pub struct UDP {
    pub header: UDPHeader,
    pub data: Vec<u8>,
}

impl FromBytes for UDP {
    fn from_bytes(bytes: Vec<u8>) -> Option<Self> {
        if bytes.len() >= mem::size_of::<UDPHeader>() {
            unsafe {
                return Option::Some(UDP {
                    header: ptr::read(bytes.as_ptr() as *const UDPHeader),
                    data: bytes[mem::size_of::<UDPHeader>().. bytes.len()].to_vec(),
                });
            }
        }
        Option::None
    }
}

impl ToBytes for UDP {
    fn to_bytes(&self) -> Vec<u8> {
        unsafe {
            let header_ptr: *const UDPHeader = &self.header;
            let mut ret = Vec::from(slice::from_raw_parts(header_ptr as *const u8, mem::size_of::<UDPHeader>()));
            ret.push_all(&self.data);
            ret
        }
    }
}

/// UDP resource
pub struct Resource {
    ip: File,
    data: Vec<u8>,
    peer_addr: IPv4Addr,
    peer_port: u16,
    host_port: u16,
}

impl Resource {
    pub fn dup(&self) -> Option<Box<Self>> {
        match self.ip.dup() {
            Some(ip) => Some(box Resource {
                ip: ip,
                data: self.data.clone(),
                peer_addr: self.peer_addr,
                peer_port: self.peer_port,
                host_port: self.host_port,
            }),
            None => None
        }
    }

    pub fn path(&self, buf: &mut [u8]) -> Option<usize> {
        let path = format!("udp://{}:{}/{}", self.peer_addr.to_string(), self.peer_port, self.host_port);

        let mut i = 0;
        for b in path.bytes() {
            if i < buf.len() {
                buf[i] = b;
                i += 1;
            } else {
                break;
            }
        }

        Some(i)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Option<usize> {
        /*

            if self.data.len() > 0 {
                let mut bytes: Vec<u8> = Vec::new();
                mem::swap(&mut self.data, &mut bytes);
                vec.push_all(&bytes);
                return Some(bytes.len());
            }

            loop {
                let mut bytes: Vec<u8> = Vec::new();
                match self.ip.read_to_end(&mut bytes) {
                    Some(_) => {
                        if let Some(datagram) = UDP::from_bytes(bytes) {
                            if datagram.header.dst.get() == self.host_port &&
                               datagram.header.src.get() == self.peer_port {
                                vec.push_all(&datagram.data);
                                return Some(datagram.data.len());
                            }
                        }
                    }
                    None => return None,
                }
            }
            */
        None
    }

    pub fn write(&mut self, buf: &[u8]) -> Option<usize> {
        let udp_data = Vec::from(buf);

        let mut udp = UDP {
            header: UDPHeader {
                src: n16::new(self.host_port),
                dst: n16::new(self.peer_port),
                len: n16::new((mem::size_of::<UDPHeader>() + udp_data.len()) as u16),
                checksum: Checksum { data: 0 },
            },
            data: udp_data,
        };

        unsafe {
            let proto = n16::new(0x11);
            let datagram_len = n16::new((mem::size_of::<UDPHeader>() + udp.data.len()) as u16);
            udp.header.checksum.data =
                Checksum::compile(Checksum::sum((&IP_ADDR as *const IPv4Addr) as usize,
                                                mem::size_of::<IPv4Addr>()) +
                                  Checksum::sum((&self.peer_addr as *const IPv4Addr) as usize,
                                                mem::size_of::<IPv4Addr>()) +
                                  Checksum::sum((&proto as *const n16) as usize,
                                                mem::size_of::<n16>()) +
                                  Checksum::sum((&datagram_len as *const n16) as usize,
                                                mem::size_of::<n16>()) +
                                  Checksum::sum((&udp.header as *const UDPHeader) as usize,
                                                mem::size_of::<UDPHeader>()) +
                                  Checksum::sum(udp.data.as_ptr() as usize, udp.data.len()));
        }

        match self.ip.write(udp.to_bytes().as_slice()) {
            Some(_) => return Some(buf.len()),
            None => return None,
        }
    }

    pub fn seek(&mut self, pos: SeekFrom) -> Option<usize> {
        None
    }

    pub fn sync(&mut self) -> bool {
        self.ip.sync()
    }
}

/// UDP scheme
pub struct Scheme;

impl Scheme {
    pub fn new() -> Box<Self> {
        box Scheme
    }

    pub fn open(&mut self, url: &str) -> Option<Box<Resource>> {
        //Split scheme from the rest of the URL
        let (scheme, mut not_scheme) = url.split_at(url.find(':').unwrap_or(url.len()));

        //Remove the starting two slashes
        if not_scheme.starts_with("//") {
            not_scheme = &not_scheme[2..not_scheme.len() - 2];
        }

        //Check host and port vs path
        if not_scheme.starts_with("/") {
            let host_port = not_scheme[1..not_scheme.len() - 1].to_string().to_num();
            if host_port > 0 && host_port < 65536 {
                if let Some(mut ip) = File::open("ip:///11") {
                    let mut bytes: Vec<u8> = Vec::new();
                    if ip.read_to_end(&mut bytes).is_some() {
                        if let Some(datagram) = UDP::from_bytes(bytes) {
                            if datagram.header.dst.get() as usize == host_port {
                                let mut url_bytes = [0; 4096];
                                if let Some(count) = ip.path(&mut url_bytes) {
                                    let url = unsafe { str::from_utf8_unchecked(&url_bytes[0..count]) };

                                    //Split scheme from the rest of the URL
                                    let (scheme, mut not_scheme) = url.split_at(url.find(':').unwrap_or(url.len()));

                                    //Remove the starting two slashes
                                    if not_scheme.starts_with("//") {
                                        not_scheme = &not_scheme[2..not_scheme.len() - 2];
                                    }

                                    let (host, port) = not_scheme.split_at(not_scheme.find(':').unwrap_or(not_scheme.len()));

                                    let peer_addr = IPv4Addr::from_string(&host.to_string());

                                    return Some(box Resource {
                                        ip: ip,
                                        data: datagram.data,
                                        peer_addr: peer_addr,
                                        peer_port: datagram.header.src.get(),
                                        host_port: host_port as u16,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let (host, port) = not_scheme.split_at(not_scheme.find(':').unwrap_or(not_scheme.len()));

            let peer_port = port.to_string().to_num();
            if peer_port > 0 && peer_port < 65536 {
                let host_port = (rand() % 32768 + 32768) as u16;

                if let Some(ip) = File::open(&format!("ip://{}/11", host)) {
                    return Some(box Resource {
                        ip: ip,
                        data: Vec::new(),
                        peer_addr: IPv4Addr::from_string(&host.to_string()),
                        peer_port: peer_port as u16,
                        host_port: host_port,
                    });
                }
            }
        }

        None
    }
}
