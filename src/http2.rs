use std::{
    error::Error,
    fmt::{Debug, Display},
    io,
    net::{SocketAddr, ToSocketAddrs},
    os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, RawFd},
    result, time,
};

use mio::{
    event::{Event, Source}, net::{TcpListener, TcpStream, UnixStream}, unix::SourceFd, Events, Interest, Poll, Token
};

#[derive(Debug)]
pub enum Http2Error {
    IOError(std::io::Error),
}

impl From<std::io::Error> for Http2Error {
    fn from(value: std::io::Error) -> Self {
        Http2Error::IOError(value)
    }
}

pub trait ListenerType = Source + AsRawFd + IntoRawFd + FromRawFd + AsFd + Debug + Sized;

pub struct Http2Server<T>
where
    T: ListenerType,
{
    token: Token,
    stream: T,
    fd: UnixStream,
}

impl<T> Http2Server<T>
where
    T: ListenerType,
{
    pub fn from_listener(listener: T, uid: usize) -> Result<Self, Http2Error> {
        let name = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        let fd = mio::net::UnixStream::connect(format!("/tmp/{}", name))?;
        Ok(Self {
            token: Token(uid),
            stream: listener,
            fd: fd,
        })
    }

    pub fn listen(&mut self)-> Result<(), Http2Error> {
        let mut poll = Poll::new()?;
        let fd = self.stream.as_raw_fd();
        let mut fd = SourceFd(&fd);
        poll.registry().register(&mut fd, self.token, Interest::READABLE)?;
        
        loop {
            let mut events =Events::with_capacity(128);
            poll.poll(&mut events, None)?;
            for event in events {
                
            }
        }

        Ok(())
    }
}

impl Http2Server<TcpListener> {
    pub fn new<A: ToSocketAddrs>(address: A, uid: usize) -> Result<Self, Http2Error> {
        for sock_addr in address.to_socket_addrs()? {
            match TcpListener::bind(sock_addr) {
                Ok(result) => {
                    let name = time::SystemTime::now()
                        .duration_since(time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        .to_string();
                    let fd = mio::net::UnixStream::connect(format!("/tmp/{}", name))?;
                    return Ok(Self {
                        token: Token(uid),
                        stream: result,
                        fd: fd,
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

impl<T> From<RawFd> for Http2Server<T>
where
    T: ListenerType,
{
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
