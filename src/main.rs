use nix::sys::socket::*;
use nix::errno::Errno;
use nix::unistd::{close, read, write};
use nix::sys::socket::sockopt::ReuseAddr;
use nix::sys::socket::accept;
use nix::sys::epoll::{epoll_create1, epoll_ctl, epoll_wait, EpollCreateFlags, EpollEvent, EpollFlags, EpollOp};
use nix::fcntl::{fcntl, OFlag, FcntlArg};
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::str::FromStr;
use std::result::Result;
use std::collections::HashMap;

const K_MAX_MSG: usize = 4096;

#[derive(PartialEq)]
enum ConnState {
    REQ,
    RES,
    END
}

struct Conn {
    fd: RawFd,
    state: ConnState,
    rbuf_size: usize,
    rbuf_received: usize,
    rbuf: [u8; 4 + K_MAX_MSG],
    wbuf_size: usize,
    wbuf_sent: usize,
    wbuf: [u8; 4 + K_MAX_MSG],
}

impl Conn {
    fn new(fd: RawFd) -> Conn {
        Conn{fd: fd, state: ConnState::REQ, rbuf_size: 0, rbuf: [0; 4 + K_MAX_MSG], rbuf_received: 0, wbuf_size: 0, wbuf_sent: 0, wbuf: [0; 4 + K_MAX_MSG]}
    }
}

fn set_nb_mode(fd: RawFd) -> Result<usize, Errno> {
    if let Err(e) = fcntl(fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)) {
        return Err(e);
    }
    Ok(0)
}

fn accept_new_conn(fd2conn: &mut HashMap<RawFd, Conn>,fd: RawFd) -> Result<usize, Errno> {
    match accept(fd) {
        Ok(connfd) => {
            fd2conn.insert(connfd, Conn::new(connfd));
        },
        Err(e) => {
            return Err(e);
        }
    }
    Ok(0)
}

fn connection_io(conn: &mut Conn) {
    match conn.state {
        ConnState::REQ => {
            state_req(conn);
        },
        ConnState::RES => {
            state_res(conn);
        }
        ConnState::END => {

        }
    }
}

fn state_req(conn: &mut Conn) {
    while try_fill_buffer(conn) {};
}

fn try_fill_buffer(conn: &mut Conn) -> bool {
    assert!(conn.rbuf_size < conn.rbuf.len());
    loop {
        match read(conn.fd, &mut conn.rbuf[conn.rbuf_size..]) {
            Ok(rv) => {
                if rv == 0 {
                    if conn.rbuf_size > 0 {
                        println!("unexpected EOF");
                    } else {
                        println!("EOF");
                    }
                    conn.state = ConnState::END;
                    return false;
                }
                conn.rbuf_size += rv;
                assert!(conn.rbuf_size <= conn.rbuf.len());
                break;
            },
            Err(e) => {
                match e {
                    Errno::EINTR => {
                        continue;
                    },
                    Errno::EAGAIN => {
                        return false;
                    },
                    _ => {
                        println!("read() error");
                        conn.state = ConnState::END;
                        return false;
                    }
                }
            }
        }
    }
    while try_one_request(conn) {}

    let remain = conn.rbuf_size - conn.rbuf_received;
    if remain > 0 {
        conn.rbuf.copy_within(conn.rbuf_received.., 0);
    }
    conn.rbuf_size = remain;
    conn.rbuf_received = 0;

    return conn.state == ConnState::REQ;
}

fn try_one_request(conn: &mut Conn) -> bool {
    if conn.rbuf_size - conn.rbuf_received < 4 {
        return false;
    }

    let mut len_buf: [u8; 4] = [0;4];
    len_buf.copy_from_slice(&conn.rbuf[conn.rbuf_received..conn.rbuf_received + 4]);
    conn.rbuf_received += 4;
    let length = u32::from_le_bytes(len_buf);

    if length > u32::try_from(K_MAX_MSG).unwrap() {
        println!("Message too long");
        conn.state = ConnState::END;
        return false;
    }

    if length > u32::try_from(conn.rbuf_size - conn.rbuf_received).unwrap() {
        return false;
    }
    let end = u32::try_from(conn.rbuf_received).unwrap() + length;
    println!("Client says {}", String::from_utf8(conn.rbuf[conn.rbuf_received..end.try_into().unwrap()].to_vec()).unwrap());

    conn.wbuf[0..4].copy_from_slice(&length.to_le_bytes());
    conn.wbuf[4..(4 + length).try_into().unwrap()].copy_from_slice(&conn.rbuf[conn.rbuf_received..end.try_into().unwrap()]);
    conn.wbuf_size = 4 + usize::try_from(length).unwrap();

    conn.rbuf_received += usize::try_from(length).unwrap();
    conn.state = ConnState::RES;

    state_res(conn);

    return conn.state == ConnState::REQ;
}

fn state_res(conn: &mut Conn) {
    while try_flush_buffer(conn) {}
}

fn try_flush_buffer(conn: &mut Conn) -> bool {
    loop {
        let remain = conn.wbuf_size - conn.wbuf_sent;
        match write(conn.fd, &mut conn.wbuf[conn.wbuf_sent..conn.wbuf_sent + remain]) {
            Ok(rv) => {
                conn.wbuf_sent += rv;
                assert!(conn.wbuf_sent <= conn.wbuf_size);
                if conn.wbuf_sent == conn.wbuf_size {
                    conn.state = ConnState::REQ;
                    conn.wbuf_size = 0;
                    conn.wbuf_sent = 0;
                    return false;
                }
                break;
            },
            Err(e) => {
                match e {
                    Errno::EINTR => {
                        continue;
                    },
                    Errno::EAGAIN => {
                        return false;
                    },
                    _ => {
                        println!("read() error");
                        conn.state = ConnState::END;
                        return false;
                    }
                }
            }
        }
    }
    return true;
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
                    let mut fd2conn: HashMap<RawFd,Conn> = HashMap::new();
                    let mut events: Vec<EpollEvent> = Vec::with_capacity(10);
                    for _ in 0..10 {
                        events.push(EpollEvent::empty());
                    }
                    if let Err(e) = set_nb_mode(fd) {
                        println!("Error {} while setting non-blocking mode on fd {}", e, fd);
                        return;
                    }
                    let epollfd = epoll_create1(EpollCreateFlags::empty()).unwrap();
                    let mut ev: EpollEvent = EpollEvent::new(EpollFlags::EPOLLIN, fd.try_into().unwrap());
                    if let Err(e) = epoll_ctl(epollfd, EpollOp::EpollCtlAdd, fd, &mut ev) {
                        println!("Error {} while calling epoll_ctl on fd {}", e, fd);
                        return;
                    }
                    loop {

                        if let Err(e) = epoll_wait(epollfd, &mut events, 1000) {
                            println!("Error {} while polling file descriptors", e);
                            return;
                        }

                        for event in events.iter() {
                            if event.data() == fd.try_into().unwrap() {
                                match accept(fd) {
                                    Ok(conn_soc) => {
                                        let _ = set_nb_mode(conn_soc);
                                        ev = EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLET, conn_soc.try_into().unwrap());
                                        if let Err(e) = epoll_ctl(epollfd, EpollOp::EpollCtlAdd, conn_soc.try_into().unwrap(), &mut ev) {
                                            println!("Error {} while calling epoll_ctl on fd {}", e, fd);
                                            return;
                                        }
                                        fd2conn.insert(conn_soc, Conn::new(conn_soc));
                                    },
                                    Err(e) => {
                                        println!("Error {} while calling epoll_ctl on fd {}", e, fd);
                                        return;
                                    }
                                }
                            } else {
                                if let Some(conn) = fd2conn.get_mut(&event.data().try_into().unwrap()) {
                                    connection_io(conn);
                                    if conn.state == ConnState::END {
                                        fd2conn.remove(&event.data().try_into().unwrap());
                                        let _ = close(event.data().try_into().unwrap());
                                    }
                                }
                            }
                        }
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
