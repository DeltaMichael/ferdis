use nix::sys::socket::*;
use nix::errno::Errno;
use std::os::fd::RawFd;
use nix::unistd::{close, read, write};
use std::str::FromStr;
use crate::server::ResType;

const K_MAX_MSG: usize = 4096;

#[derive(Debug)]
pub struct FerdisResponse {
    pub res_type: ResType,
    pub res_code: u32,
    pub message: Option<String>
}

// Guard against partial writes
fn write_full(fd: RawFd, wbuf: &mut[u8]) -> Result<usize, Errno> {
    let mut buf_start = 0;
    let mut n = wbuf.len();
    while n > 0 {
        match write(fd, &mut wbuf[buf_start..]) {
            Ok(rv) => {
                if rv <= 0 {
                    println!("Zero bytes written");
                    return Err(Errno::EIO);
                }
                assert!(rv <= n);
                n -= rv;
                buf_start += rv;
            },
            Err(e) => {
                println!("Error while writing {}", e);
                return Err(e);
            }
        }
    }
    Ok(n)
}

// Guard against partial reads
fn read_full(fd: RawFd, rbuf: &mut[u8]) -> Result<usize, Errno> {
    let mut buf_start = 0;
    let mut n = rbuf.len();
    while n > 0 {
        match read(fd, &mut rbuf[buf_start..]) {
            Ok(rv) => {
                if rv <= 0 {
                    println!("Zero bytes read");
                    return Err(Errno::EIO);
                }
                assert!(rv <= n);
                n -= rv;
                buf_start += rv;
            }, Err(e) => {
                println!("Error while reading {}", e);
                return Err(e);
            }
        }
    }
    Ok(n)
}

fn send_request(fd: RawFd, text: &str) -> Result<usize, Errno> {
    let reply: &[u8] = text.as_bytes();
    let mut wbuf = [0; K_MAX_MSG];
    let length = u32::try_from(reply.len()).unwrap();
    wbuf[0..4].copy_from_slice(&length.to_le_bytes());
    wbuf[4..4 + reply.len()].copy_from_slice(reply);
    write_full(fd, &mut wbuf[0..4 + reply.len()])
}

fn read_response(fd: RawFd) -> Result<FerdisResponse, Errno> {
    let mut len_buf: [u8; 4] = [0; 4];
    let mut length;
    let mut rbuf: [u8; K_MAX_MSG] = [0; K_MAX_MSG];
    match read_full(fd, &mut len_buf) {
        Ok(_) => {
            length = u32::from_le_bytes(len_buf);
        },
        Err(e) => {
            println!("read() error {}", e);
            return Err(e);
        }
    }
    let response;
    match read_full(fd, &mut rbuf[..length.try_into().unwrap()]) {
        Ok(_) => {
            response = deserialize_response(&mut rbuf[..length.try_into().unwrap()]);
            return Ok(response);
        }
        Err(e) => {
            println!("read() error {}", e);
            return Err(e);
        }
    }
}

pub fn deserialize_response(rbuf: &mut[u8]) -> FerdisResponse {
    let res_type_u32 = deserialize_u32(&mut rbuf[0..4]);
    let res_type = ResType::from_u32(res_type_u32);
    match res_type {
        ResType::NIL => {
            return FerdisResponse{res_type: res_type, res_code: 0, message: None};
        },
        ResType::ERR => {
            let err_code = deserialize_u32(&mut rbuf[4..8]);
            let message_length = deserialize_u32(&mut rbuf[8..12]);
            let message = deserialize_string(&mut rbuf[12..], usize::try_from(message_length).unwrap());
            return FerdisResponse{res_type: res_type, res_code: err_code, message: Some(message) };
        },
        ResType::STR => {
            let message_length = deserialize_u32(&mut rbuf[4..8]);
            let message = deserialize_string(&mut rbuf[8..], usize::try_from(message_length).unwrap());
            return FerdisResponse{res_type: res_type, res_code: 0, message: Some(message) };
        },
        ResType::ARR => {
            panic!("Implement arrays");
        },
    }
}

pub fn deserialize_u32(rbuf: &mut[u8]) -> u32 {
    let mut buf: [u8; 4] = [0; 4];
    buf.copy_from_slice(&rbuf[0..4]);
    let val = u32::from_le_bytes(buf);
    return val;
}

pub fn deserialize_string(rbuf: &mut[u8], length: usize) -> String {
    String::from_utf8(rbuf[..length.try_into().unwrap()].to_vec()).unwrap()
}

pub fn send_message(req: String) -> Result<FerdisResponse, Errno> {
    let fd = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None);
    match fd {
        Ok(fd) => {
            let localhost = SockaddrIn::from_str("127.0.0.1:8081").unwrap();
            match connect(fd, &localhost) {
                Ok(()) => {
                    if let Err(e) = send_request(fd, &req) {
                        println!("Error {} sending request {}", e, req);
                        let _ = close(fd);
                        return Err(e);
                    }
                },
                Err(e) => {
                    println!("Error connecting to server {}", e);
                    return Err(e);
                }
            }
            let res = read_response(fd);
            let _ = close(fd);
            return res;
        },
        Err(e) => {
            println!("Error opening socket {}", e);
            return Err(e);
        }
    };
}
