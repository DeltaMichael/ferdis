use nix::sys::socket::*;
use nix::errno::Errno;
use nix::unistd::{close, read, write};
use nix::sys::socket::sockopt::ReuseAddr;
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::str::FromStr;
use std::result::Result;

const K_MAX_MSG: usize = 8;

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
                            return Ok(buf_start);
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
    Ok(buf_start)
}

enum ParseResult {
    Length,
    RestOfMessage(usize),
    PartialLength(Vec<u8>)
}

fn one_write(confd: RawFd, text: &str) -> Result<usize, Errno> {
    let reply: &[u8] = text.as_bytes();
    let mut wbuf = [0; K_MAX_MSG];
    let length = u32::try_from(reply.len()).unwrap();
    wbuf[0..4].copy_from_slice(&length.to_le_bytes());
    wbuf[4..4 + reply.len()].copy_from_slice(reply);
    write_full(confd, &mut wbuf[0..4 + reply.len()])
}

fn one_request(confd: RawFd, prev_result: &mut ParseResult) -> Result<ParseResult, Errno> {
    let mut rbuf: [u8; 4 + K_MAX_MSG] = [0; 4 + K_MAX_MSG];
    let mut len_bytes: [u8; 4] = [0;4];
    match read_full(confd, &mut rbuf) {
        Ok(n) => {
            if n == 0 {
                return Err(Errno::UnknownErrno);
            }
            let mut start = 0;
            while n > start {
                match prev_result {
                    ParseResult::Length => {
                        if rbuf[start..].len() <= 4 {
                            return Ok(ParseResult::PartialLength(rbuf[start..].to_vec()));
                        }
                        len_bytes.clone_from_slice(&rbuf[start..start + 4]);
                        let length = u32::from_le_bytes(len_bytes);
                        start += 4;
                        print!("Client says ");
                        if usize::try_from(length).unwrap() > rbuf[start..].len() {
                            print!("{}", String::from_utf8(rbuf[start..].to_vec()).unwrap());
                            return Ok(ParseResult::RestOfMessage(usize::try_from(length).unwrap() - rbuf[start..].len()));
                        } else {
                            print!("{}\n", String::from_utf8(rbuf[start..start + usize::try_from(length).unwrap()].to_vec()).unwrap());
                            // let _ = one_write(confd, "world");
                        }
                        start += usize::try_from(length).unwrap();
                    }
                    ParseResult::RestOfMessage(s) => {
                        if *s < rbuf.len() {
                            print!("{}\n", String::from_utf8(rbuf[..*s].to_vec()).unwrap());
                            start += *s;
                            *prev_result = ParseResult::Length;
                            // let _ = one_write(confd, "world");
                        } else {
                            print!("{}", String::from_utf8(rbuf.to_vec()).unwrap());
                            return Ok(ParseResult::RestOfMessage(*s - rbuf.len()));
                        }
                    },
                    ParseResult::PartialLength(len_buf) => {
                        for (i, byte) in len_buf.iter().enumerate() {
                            len_bytes[i] = *byte;
                        }
                        let offset = len_buf.len();
                        for i in 0..4 - len_buf.len() {
                            len_bytes[offset + i] = rbuf[i];
                            start += 1;
                        }
                        let length = u32::from_le_bytes(len_bytes);
                        *prev_result = ParseResult::RestOfMessage(usize::try_from(length).unwrap());
                    }
                }
            }
        },
        Err(e) => {
            return Err(e);
        }
    }
    Ok(ParseResult::Length)
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
                                let mut prev_result = ParseResult::Length;
                                loop {
                                    match one_request(confd, &mut prev_result) {
                                        Ok(l) => {
                                            prev_result = l;
                                        },
                                        Err(e) => {
                                            println!("error occurred {}", e);
                                            break;
                                        }
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
