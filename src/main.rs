use nix::sys::socket::*;
use nix::errno::Errno;
use nix::unistd::{close, read, write};
use nix::sys::socket::sockopt::ReuseAddr;
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::str::FromStr;
use std::result::Result;

const K_MAX_MSG: usize = 4096;

// Guard against partial writes
fn write_full(fd: RawFd, wbuf: &mut[u8]) -> Result<usize, Errno> {
    let mut buf_start = 0;
    let mut n = wbuf.len();
    while n > 0 {
        match write(fd, &mut wbuf[buf_start..]) {
            Ok(rv) => {
                if rv <= 0 {
                    match Errno::last() {
                        Errno::UnknownErrno => {
                            println!("EOF");
                            return Err(Errno::UnknownErrno);
                        },
                        e => {
                            return Err(e);
                        }
                    }
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
                    match Errno::last() {
                        Errno::UnknownErrno => {
                            println!("EOF");
                            return Err(Errno::UnknownErrno);
                        },
                        e => {
                            return Err(e);
                        }
                    }
                }
                assert!(rv <= n);
                n -= rv;
                buf_start += rv;
            },
            Err(e) => {
                println!("Error while reading {}", e);
                return Err(e);
            }
        }
    }
    Ok(n)
}
fn one_request(confd: RawFd) -> Result<usize, Errno> {
    let mut len_buf: [u8; 4] = [0; 4];
    let length;
    let mut rbuf: [u8; K_MAX_MSG] = [0; K_MAX_MSG];
    match read_full(confd, &mut len_buf) {
        Ok(_) => {
            length = u32::from_le_bytes(len_buf);
        },
        Err(e) => {
            if Errno::last() != Errno::UnknownErrno {
                println!("read() error {}", e);
            }
            return Err(e);
        }
    }

    match read_full(confd, &mut rbuf[..length.try_into().unwrap()]) {
        Ok(_) => {
            println!("Client says {}", String::from_utf8(rbuf[..length.try_into().unwrap()].to_vec()).unwrap());
        }
        Err(e) => {
            if Errno::last() != Errno::UnknownErrno {
                println!("read() error {}", e);
            }
            return Err(e);
        }
    }

    let reply: &[u8] = "world".as_bytes();
    let mut wbuf = [0; K_MAX_MSG];
    let length = u32::try_from(reply.len()).unwrap();
    wbuf[0..4].copy_from_slice(&length.to_le_bytes());
    wbuf[4..4 + reply.len()].copy_from_slice(reply);
    write_full(confd, &mut wbuf[0..4 + reply.len()])
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
                                    if let Err(_) = one_request(confd) {
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
