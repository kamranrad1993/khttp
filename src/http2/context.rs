use std::{io, result};

use http::{request, Request, Response};
use kparser::http2::HpackContext;
use mio::event::Source;

use super::TcpStream;

pub enum ContextError {
    IOError(io::Error),
    IncompleteStream
}

pub struct Http2Context
{
    stream: TcpStream,
    hpack_context: HpackContext,
    read_buffer: Vec<u8>,// pure binary data that have been read from stream
    write_buffer: Vec<u8>,// binary encoded http2 frames that will be writed to stream
}

impl Source for Http2Context
{
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.register(&mut self.stream, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.reregister(&mut self.stream, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        registry.deregister(&mut self.stream)
    }
}

impl Http2Context
{
    pub fn new(stream: TcpStream, mut max_header: Option<usize>) -> Self {
        if max_header.is_none() {
            max_header = Some(128);
        }
        Self {
            hpack_context: HpackContext::new(max_header.unwrap()),
            stream: stream,
            read_buffer: Vec::new(),
            write_buffer: Vec::new()
        }
    }

    // pub fn read_req(&mut self) -> Result<Request<Vec<u8>>, ContextError> {
        
    // }

    // pub fn write_req(&mut self, req: Request<Vec<u8>>) -> Result<(), ContextError> {}

    // pub fn read_res(&mut self) -> Result<Response<Vec<u8>>, ContextError> {}

    // pub fn write_res(&mut self, req: Response<Vec<u8>>) -> Result<(), ContextError> {}
}
