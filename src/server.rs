use nix::sys::socket::*;
use nix::errno::Errno;
use nix::unistd::{close, read, write};
use nix::sys::socket::sockopt::ReuseAddr;
use nix::sys::socket::accept;
use nix::poll::PollFd;
use nix::poll::PollFlags;
use nix::poll::poll;
use nix::fcntl::{fcntl, OFlag, FcntlArg};
use std::os::fd::AsRawFd;
use std::os::fd::RawFd;
use std::str::FromStr;
use std::result::Result;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::oa_map::OAMap;

const K_MAX_MSG: usize = 4096;
use once_cell::sync::Lazy;

static STORAGE: Lazy<Mutex<OAMap<String, String>>> = Lazy::new(|| {
    let m = OAMap::new();
    Mutex::new(m)
});

#[derive(PartialEq)]
enum ConnState {
    REQ,
    RES,
    END
}

#[derive(Debug)]
pub enum ResType {
   NIL = 0,
   ERR = 1,
   STR = 2,
   ARR = 3
}

impl ResType {

    pub fn as_str(&self) -> String {
        match self {
            ResType::NIL => {
                String::from("NIL")
            },
            ResType::ERR => {
                String::from("ERR")
            },
            ResType::STR => {
                String::from("STR")
            },
            ResType::ARR => {
                String::from("ARR")
            },
        }
    }

    pub fn from_u32(value: u32) -> ResType {
        match value {
            0 => {
                return ResType::NIL;
            },
            1 => {
                return ResType::ERR;
            },
            2 => {
                return ResType::STR;
            },
            3 => {
                return ResType::ARR;
            },
            _ => {
                panic!("Unknown value {}", value);
            }
        }
    }
}

struct Conn {
    fd: RawFd,
    state: ConnState,
    rbuf_size: usize,
    rbuf: [u8; 4 + K_MAX_MSG],
    wbuf_size: usize,
    wbuf_sent: usize,
    wbuf: [u8; 4 + K_MAX_MSG],
}

struct Response {
    length: u32,
    message: Vec<u8>
}

impl Conn {
    fn new(fd: RawFd) -> Conn {
        Conn{fd: fd, state: ConnState::REQ, rbuf_size: 0, rbuf: [0; 4 + K_MAX_MSG], wbuf_size: 0, wbuf_sent: 0, wbuf: [0; 4 + K_MAX_MSG]}
    }
}

fn set_nb_mode(fd: RawFd) -> Result<usize, Errno> {
    if let Err(e) = fcntl(fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)) { return Err(e);
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
                        println!("read() error while filling buffer");
                        conn.state = ConnState::END;
                        return false;
                    }
                }
            }
        }
    }
    while try_one_request(conn) {}
    return conn.state == ConnState::REQ;
}

fn do_request(req_buf: &[u8]) -> Result<Response, Errno> {
    match parse_request(req_buf) {
        Ok(r) => {
            let command: Vec<&str> = r.split(" ").collect();
            match command[0] {
                "get" => {
                    // do get
                    return do_get(command);
                },
                "set" => {
                    // do set
                    return do_set(command);
                },
                "del" => {
                    // do del
                    return do_del(command);
                },
                "keys" => {
                    // do keys
                    return do_keys(command);
                },
                _ => {
                    let out = out_err(1, "Unknown command");
                    return Ok(Response { length: u32::try_from(out.len()).unwrap(), message: out});
                }
            }
        },
        Err(e) => {
            return Err(e);
        }
    }
}
fn do_keys(command: Vec<&str>) -> Result<Response,Errno> {
    if command.len() > 1 {
        let out = out_err(3, "Too many arguments");
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }

    let storage = STORAGE.lock().unwrap();
    let keys = storage.keys();
    let out;
    if keys.is_empty() {
        out = out_nil();
    } else {
        out = out_arr(storage.keys());
    }
    return Ok(Response { length: u32::try_from(out.len()).unwrap(), message: out });
}

fn do_get(command: Vec<&str>) -> Result<Response,Errno> {
    if command.len() < 2 {
        let out = out_err(2, "Insufficient arguments");
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }

    let storage = STORAGE.lock().unwrap();
    if !storage.contains_key(command[1].to_string()) {
        let out = out_nil();
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }
    let value = storage.get(command[1].to_string()).unwrap();
    let out = out_str(&value);

    return Ok(Response { length: u32::try_from(out.len()).unwrap(), message: out });
}

fn do_set(command: Vec<&str>) -> Result<Response,Errno> {
    if command.len() < 3 {
        let out = out_err(2, "Insufficient arguments");
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }
    let mut storage = STORAGE.lock().unwrap();
    storage.put(command[1].to_string(), command[2].to_string());
    let out = out_nil();
    Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out})
}

fn do_del(command: Vec<&str>) -> Result<Response,Errno> {
    if command.len() < 2 {
        let out = out_err(2, "Insufficient arguments");
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }

    let mut storage = STORAGE.lock().unwrap();
    if storage.contains_key(command[1].to_string()) {
        let val = storage.get(command[1].to_string());
        let out = out_str(&val.unwrap());
        storage.delete(command[1].to_string());
        return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
    }
    let out = out_nil();
    return Ok(Response {length: u32::try_from(out.len()).unwrap(), message: out});
}

fn out_nil() -> Vec<u8> {
    let mut out = Vec::new();
    (ResType::NIL as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    return out;
}

fn out_str(val: &str) -> Vec<u8> {
    let mut out = Vec::new();
    (ResType::STR as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    (val.len() as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    val.bytes().for_each(|b| out.push(b));
    return out;
}

fn out_err(code: u32, message: &str) -> Vec<u8> {
    let mut out = Vec::new();
    (ResType::ERR as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    code.to_le_bytes().iter().for_each(|b| out.push(*b));
    (message.len() as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    message.bytes().for_each(|b| out.push(b));
    return out;
}

fn out_arr(values: Vec<String>) -> Vec<u8> {
    let mut out = Vec::new();
    (ResType::ARR as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    (values.len() as u32).to_le_bytes().iter().for_each(|b| out.push(*b));
    for val in values {
        out_str(&val).iter().for_each(|b| out.push(*b));
    }
    return out;
}

fn parse_request(req_buf: &[u8]) -> Result<String, Errno> {
    Ok(String::from_utf8(req_buf.to_vec()).unwrap())
}

fn try_one_request(conn: &mut Conn) -> bool {
    if conn.rbuf_size < 4 {
        // not enough data in the buffer, retry
        return false;
    }

    let mut len_buf: [u8; 4] = [0;4];
    len_buf.copy_from_slice(&conn.rbuf[0..4]);
    let length = u32::from_le_bytes(len_buf);

    if length > u32::try_from(K_MAX_MSG).unwrap() {
        println!("Message too long");
        conn.state = ConnState::END;
        return false;
    }

    if 4 + length > u32::try_from(conn.rbuf_size).unwrap() {
        // not enough data in the buffer, retry
        return false;
    }

    println!("Client says {}", String::from_utf8(conn.rbuf[4..(4 + length).try_into().unwrap()].to_vec()).unwrap());
    // get one request and generate a response
    match do_request(&conn.rbuf[4..4 + usize::try_from(length).unwrap()]) {
        Ok(res) => {
            conn.wbuf[0..4].copy_from_slice(&res.length.to_le_bytes());
            conn.wbuf[4..4 + usize::try_from(res.length).unwrap()].copy_from_slice(&res.message);
            conn.wbuf_size += 4 + usize::try_from(res.length).unwrap();
        },
        Err(_) => {
            println!("Could not do request");
            conn.state = ConnState::END;
            return false;
        }
    }


    // remove the request from the buffer
    let remain = conn.rbuf_size - 4 - usize::try_from(length).unwrap();
    if remain > 0 {
        conn.rbuf.copy_within(4 + usize::try_from(length).unwrap().., 0);
    }
    conn.rbuf_size = remain;

    // change state
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
                        println!("read() error while flushing buffer");
                        conn.state = ConnState::END;
                        return false;
                    }
                }
            }
        }
    }
    return true;
}

pub fn run_server() {
    let fd = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None);
    match fd {
        Ok(fd) => {
            let _ = setsockopt(fd, ReuseAddr, &true);
            let localhost = SockaddrIn::from_str("0.0.0.0:8081").unwrap();
            bind(fd.as_raw_fd(), &localhost).expect("bind");
            match listen(fd.as_raw_fd(), 128) {
                Ok(()) => {
                    let mut fd2conn: HashMap<RawFd,Conn> = HashMap::new();
                    let mut poll_args: Vec<PollFd> = Vec::new();
                    let mut listening_id: PollFd;
                    if let Err(e) = set_nb_mode(fd) {
                        println!("Error {} while setting non-blocking mode on fd {}", e, fd);
                        return;
                    }
                    loop {
                        poll_args.clear();
                        listening_id = PollFd::new(fd, PollFlags::POLLIN);
                        for (fd, conn) in fd2conn.iter() {
                            let mut pfd = PollFd::new(*fd, PollFlags::empty());
                            if conn.state == ConnState::REQ {
                                pfd.set_events(PollFlags::POLLERR | PollFlags::POLLIN);
                            } else {
                                pfd.set_events(PollFlags::POLLERR | PollFlags::POLLOUT);
                            }
                            poll_args.push(pfd);
                        }
                        if let Err(e) = poll(&mut poll_args, 1000) {
                            println!("Error {} while polling file descriptors", e);
                            return;
                        }

                        for poll_fd in poll_args.iter() {
                            let conn = fd2conn.get_mut(&poll_fd.as_raw_fd()).unwrap();
                            connection_io(conn);
                            if conn.state == ConnState::END {
                                fd2conn.remove(&poll_fd.as_raw_fd());
                                let _ = close(poll_fd.as_raw_fd());
                            }
                        }

                        if let Some(_) = listening_id.revents() {
                            let _ = accept_new_conn(&mut fd2conn, fd);
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
