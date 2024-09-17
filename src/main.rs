use nix::sys::socket::*;
use nix::errno::Errno;
use nix::unistd::{close, read, write};
use nix::sys::socket::sockopt::ReuseAddr;
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::str::FromStr;
use std::result::Result;

const K_MAX_MSG: usize = 4096;

fn one_request(confd: RawFd) -> Result<usize, Errno> {
    let mut len_buf: [u8; 4] = [0; 4];
    let length;
    let mut rbuf: [u8; K_MAX_MSG] = [0; K_MAX_MSG];
    match read(confd, &mut len_buf) {
        Ok(rv) => {
            length = u32::from_le_bytes(len_buf);
            if rv == 0 {
                return Err(Errno::EIO);
            }
        },
        Err(e) => {
            println!("read() error {}", e);
            return Err(e);
        }
    }

    match read(confd, &mut rbuf[..length.try_into().unwrap()]) {
        Ok(rv) => {
            if rv == 0 {
                return Err(Errno::EIO);
            }
            println!("Client says {}", String::from_utf8(rbuf[..length.try_into().unwrap()].to_vec()).unwrap());
        }
        Err(e) => {
            println!("read() error {}", e);
            return Err(e);
        }
    }

    let reply: &[u8] = "world".as_bytes();
    let mut wbuf = [0; K_MAX_MSG];
    let length = u32::try_from(reply.len()).unwrap();
    wbuf[0..4].copy_from_slice(&length.to_le_bytes());
    wbuf[4..4 + reply.len()].copy_from_slice(reply);
    match write(confd, &mut wbuf[0..4 + reply.len()]) {
        Ok(_) => {
            Ok(0)
        },
        Err(e) => {
            println!("Error while writing {}", e);
            return Err(e);
        }
    }
}

fn main() {
    let fd = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None);
    match fd {
        Ok(fd) => {
            let _ = setsockopt(fd, ReuseAddr, &true);
            let localhost = SockaddrIn::from_str("0.0.0.0:8081").unwrap();
            bind(fd.as_raw_fd(), &localhost).expect("bind");
            match listen(fd.as_raw_fd(), 128) {
                Ok(()) => {
                    loop {
                        let confd = accept(fd.as_raw_fd());
                        match confd {
                            Ok(confd) => {
                                loop {
                                    if let Err(e) = one_request(confd) {
                                        break;
                                    }
                                }
                            },
                            Err(_) => {
                                continue;
                            }
                        }
                        let _ = close(confd.unwrap());
                    }
                },
                Err(e) => {
                    println!("Error while calling listen {}", e);
                }
            }
        },
        Err(e) => {
            println!("Error while opening socket {}", e);
        }
    }
}
