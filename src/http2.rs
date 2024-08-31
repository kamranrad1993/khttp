
use std::{
    fmt::{Debug, Display},
    net::ToSocketAddrs,
    os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, RawFd},
};

use mio::{
    event::Source,
    net::{TcpListener, TcpStream},
    Poll,
};

trait ListenerType<T> = where T :
   Source + AsRawFd + IntoRawFd + FromRawFd + AsFd + Debug + Display + Sized + Clone + Copy;

pub struct Http2Server<T>
where
    T: ListenerType,
{
    stream: T,
}

// ToSocketAddrs
impl<T> Http2Server<T>
where
    T: ListenerType,
{
    pub fn new<A: ToSocketAddrs>() -> Self {
        let f: RawFd = 0 as RawFd;
        let l = <Http2Server<TcpStream> as From<RawFd>>::from(f);

        todo!()
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
