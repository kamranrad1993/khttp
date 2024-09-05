use std::{
    collections::HashMap,
    io::{self, Read, Write},
    ptr::read,
    result,
};

use http::{request, Request, Response};
use kparser::{
    http2::{frame, Frame, FrameParseError, HpackContext, Len, SETTINGS_HEADER_TABLE_SIZE},
    u31::u31,
    Http2Pri,
};
use mio::event::Source;

use crate::BUFFER_SIZE;

use super::{Http2Stream, TcpStream};

pub enum ContextError {
    IOError(io::Error),
    IncompleteStream,
    ClientDisconnected,
    NotHttp2,
    NoDataReady,
    InvalidStream,
}

impl From<io::Error> for ContextError {
    fn from(value: io::Error) -> Self {
        ContextError::IOError(value)
    }
}

impl From<FrameParseError> for ContextError {
    fn from(value: FrameParseError) -> Self {
        match value {
            FrameParseError::InsufficentLength | FrameParseError::InsufficentPayloadLength => {
                return ContextError::IncompleteStream
            }
            FrameParseError::PayloadParseError(e) => return ContextError::InvalidStream,
        }
    }
}

pub struct Http2Context {
    handshaked: bool,
    buffer_size: usize,
    connection: TcpStream,
    hpack_context: HpackContext,
    streams: HashMap<u31, Http2Stream>,
    read_buffer: Vec<u8>,
}

impl Source for Http2Context {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.register(&mut self.connection, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        registry.reregister(&mut self.connection, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        registry.deregister(&mut self.connection)
    }
}

impl Http2Context {
    pub fn new(
        stream: TcpStream,
        mut max_header: Option<usize>,
        mut buffer_size: Option<usize>,
    ) -> Self {
        if max_header.is_none() {
            max_header = Some(128);
        }
        if buffer_size.is_none() {
            buffer_size = Some(4096);
        }

        Self {
            handshaked: false,
            buffer_size: buffer_size.unwrap(),
            hpack_context: HpackContext::new(max_header.unwrap()),
            connection: stream,
            streams: HashMap::new(),
            read_buffer: Vec::new(),
        }
    }

    pub fn handle_read(&mut self) -> Result<(), ContextError> {
        let mut buffer = vec![0u8; self.buffer_size];
        match self.connection.read(&mut buffer) {
            Ok(read_size) => {
                if read_size == 0 {
                    return Err(ContextError::ClientDisconnected);
                }
                self.read_buffer.extend(&buffer[0..read_size]);

                if !self.handshaked {
                    if let Err(e) = Http2Pri::read_and_remove(&mut self.read_buffer) {
                        return Err(ContextError::NotHttp2);
                    }
                    self.handshaked = true;
                }

                loop {
                    if self.read_buffer.len() == 0 {
                        return Ok(());
                    }
                    let (frame_size, frame) = self.read_frame(&self.read_buffer)?;
                    self.read_buffer.drain(0..frame_size);
                    self.handle_frame(frame);
                }
            }
            Err(e) => match e.kind() {
                io::ErrorKind::ConnectionRefused
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::NotConnected
                | io::ErrorKind::UnexpectedEof
                | io::ErrorKind::BrokenPipe => return Err(ContextError::ClientDisconnected),
                io::ErrorKind::WouldBlock => return Err(ContextError::NoDataReady),
                io::ErrorKind::PermissionDenied
                | io::ErrorKind::AddrInUse
                | io::ErrorKind::AddrNotAvailable
                | io::ErrorKind::InvalidInput
                | io::ErrorKind::AlreadyExists
                | io::ErrorKind::InvalidData
                | io::ErrorKind::WriteZero
                | io::ErrorKind::Interrupted
                | io::ErrorKind::Unsupported
                | io::ErrorKind::OutOfMemory
                | io::ErrorKind::Other
                | _ => return Err(ContextError::IOError(e)),
            },
        }
    }

    fn read_frame(&self, buf: &Vec<u8>) -> Result<(usize, Frame), ContextError> {
        let mut frame = <Frame as TryFrom<&[u8]>>::try_from(&buf)?;
        let len = <Frame as Len>::binary_len(&frame);
        Ok((len, frame))
    }

    fn handle_frame(&mut self, frame: Frame) {
        let stream = match self.streams.get_mut(&frame.stream_id) {
            Some(stream) => stream,
            None => {
                let mut stream = Http2Stream::new(frame.stream_id);
                self.streams.insert(frame.stream_id, stream);
                self.streams.get_mut(&frame.stream_id).unwrap()
            }
        };

        match frame.payload {
            kparser::http2::Payload::Settings(settings_payload) => {
                for (id, value) in settings_payload.settings {
                    match id {
                        SETTINGS_HEADER_TABLE_SIZE => {
                            // resize hpack context here
                        },
                        SETTINGS_ENABLE_PUSH => {},
                        SETTINGS_MAX_CONCURRENT_STREAMS => {},
                        SETTINGS_INITIAL_WINDOW_SIZE => {},
                        SETTINGS_MAX_FRAME_SIZE => {},
                        SETTINGS_MAX_HEADER_LIST_SIZE => {},
                        _ => {},
                    }
                }
            }
            kparser::http2::Payload::Data(_) => todo!(),
            kparser::http2::Payload::Headers(_) => todo!(),
            kparser::http2::Payload::Priority(_) => todo!(),
            kparser::http2::Payload::RstStream(_) => todo!(),
            kparser::http2::Payload::PushPromise(_) => todo!(),
            kparser::http2::Payload::Ping(_) => todo!(),
            kparser::http2::Payload::GoAway(_) => todo!(),
            kparser::http2::Payload::WindowUpdate(_) => todo!(),
            kparser::http2::Payload::Continuation(_) => todo!(),
        }
    }
}
