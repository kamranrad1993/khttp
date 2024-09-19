pub mod context;
pub mod stream;
use kparser::u31::u31;
pub use stream::*;

use std::{
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, ToSocketAddrs},
    os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, RawFd},
    rc::Weak,
    result, time,
};

use context::Http2Context;
use id_pool::IdPool;
use mio::{
    event::{Event, Source},
    net::{TcpListener, TcpStream, UnixStream},
    unix::SourceFd,
    Events, Interest, Poll, Registry, Token,
};

#[derive(Debug)]
pub enum Http2Error {
    IOError(std::io::Error),
    MaxActiveConnection,
}

impl From<std::io::Error> for Http2Error {
    fn from(value: std::io::Error) -> Self {
        Http2Error::IOError(value)
    }
}

const LISTENRE_TOKEN: Token = Token(0);

pub struct Http2Server {
    listener: TcpListener,
    connections: HashMap<Token, Http2Context>,
}

impl Http2Server {
    pub fn from_listener(listener: TcpListener) -> Result<Self, Http2Error> {
        let name = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        let fd = mio::net::UnixStream::connect(format!("/tmp/{}", name))?;
        Ok(Self {
            listener,
            connections: HashMap::new(),
        })
    }

    pub fn listen(&mut self) -> Result<(), Http2Error> {
        let mut poll = Poll::new()?;
        let fd = self.listener.as_raw_fd();
        let mut fd = SourceFd(&fd);
        poll.registry()
            .register(&mut fd, LISTENRE_TOKEN, Interest::READABLE)?;

        let mut id_pool = IdPool::new();

        loop {
            let mut events = Events::with_capacity(128);
            poll.poll(&mut events, None)?;
            for event in &events {
                match event.token() {
                    LISTENRE_TOKEN => {
                        if event.is_readable() {
                            let (mut tcp_stream, addr) = self.listener.accept()?;
                            let id =
                                match id_pool.request_id().ok_or(Http2Error::MaxActiveConnection) {
                                    Ok(id) => id,
                                    Err(e) => {
                                        eprintln!("Max Active Connection Reached");
                                        let _ = tcp_stream
                                            .write("Max Active Connection Reached".as_bytes());
                                        let _ = tcp_stream.shutdown(Shutdown::Both);
                                        continue;
                                    }
                                };
                            let token = Token(id);
                            let mut context = Http2Context::new(tcp_stream, None, None);
                            poll.registry().register(
                                &mut context,
                                token,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;
                            self.connections.insert(token, context);
                        }
                    }
                    token => {
                        let context = match self.connections.get_mut(&token) {
                            Some(context) => context,
                            None => {
                                self.connections.remove(&token);
                                eprintln!("Now Such Context Found");
                                continue;
                            }
                        };

                        if event.is_readable() {
                            match context.handle_read(false) {
                                Ok(streams) => {
                                    for stream in streams {
                                        println!("new stream");
                                        println!("{}", stream);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error : {}", e);
                                    poll.registry().deregister(context)?;
                                    self.connections.remove(&token);
                                }
                            }
                        }

                        if event.is_writable() {}
                    }
                }
            }
        }

        Ok(())
    }
}

impl Http2Server {
    pub fn new<A: ToSocketAddrs>(address: A) -> Result<Self, Http2Error> {
        for sock_addr in address.to_socket_addrs()? {
            match TcpListener::bind(sock_addr) {
                Ok(result) => {
                    return Ok(Self {
                        listener: result,
                        connections: HashMap::new(),
                    });
                }
                Err(e) => return Err(Http2Error::IOError(e)),
            };
        }
        Err(Http2Error::IOError(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "No Valid Address where Found",
        )))
    }
}

impl From<RawFd> for Http2Server {
    fn from(value: RawFd) -> Self {
        todo!()
    }
}

// impl<T> Source for Http2Server<T>
// where
//     T: ListenerType,
// {
//     fn register(
//         &mut self,
//         registry: &mio::Registry,
//         token: mio::Token,
//         interests: mio::Interest,
//     ) -> io::Result<()> {
//     }

//     fn reregister(
//         &mut self,
//         registry: &mio::Registry,
//         token: mio::Token,
//         interests: mio::Interest,
//     ) -> io::Result<()> {
//         todo!()
//     }

//     fn deregister(&mut self, registry: &mio::Registry) -> io::Result<()> {
//         todo!()
//     }
// }
