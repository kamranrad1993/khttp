use std::{
    cell::RefCell, collections::HashMap, fmt::Display, io::{self, Read, Write}, ptr::read, rc::Rc, result, sync::Arc
};

use http::{request, Request, Response};
use kparser::{
    http2::{
        frame, ContinuationPayloadFlag, DataPayload, DataPayloadFlag, Frame, FrameParseError,
        HeadersPayloadFlag, HpackContext, HpackError, Len, SETTINGS_HEADER_TABLE_SIZE,
    },
    u31::u31,
    Http2Pri,
};
use mio::event::Source;

use crate::BUFFER_SIZE;

use super::{stream, Http2Stream, StreamState, TcpStream};

#[derive(Debug)]
pub enum ContextError {
    IOError(io::Error),
    HeaderDecodeError(HpackError),
    IncompleteStream,
    ClientDisconnected,
    NotHttp2,
    NoDataReady,
    InvalidStream,
    MaxHeaderLenExceeded,
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

impl From<HpackError> for ContextError {
    fn from(value: HpackError) -> Self {
        ContextError::HeaderDecodeError(value)
    }
}

impl Display for ContextError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextError::IOError(e) => f.write_str(format!("{}", e).as_str()),
            ContextError::HeaderDecodeError(e) => f.write_str("Hpack Error"),
            ContextError::IncompleteStream => f.write_str("ContextError::IncompleteStream"),
            ContextError::ClientDisconnected => f.write_str("ContextError::ClientDisconnected"),
            ContextError::NotHttp2 => f.write_str("ContextError::NotHttp2"),
            ContextError::NoDataReady => f.write_str("ContextError::NoDataReady"),
            ContextError::InvalidStream => f.write_str("ContextError::InvalidStream"),
            ContextError::MaxHeaderLenExceeded => f.write_str("ContextError::MaxHeaderLenExceeded"),
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
    enable_push: bool,
    max_streams: u32,
    nax_window_size: u128,
    max_frame_size: u32,
    max_headers_len: u32,
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
            enable_push: true,
            max_streams: 0,
            nax_window_size: 65535,
            max_frame_size: 16384,
            max_headers_len: 0,
        }
    }

    pub fn handle_read(
        &mut self,
        read_data_stream: bool,
    ) -> Result<Vec<Http2Stream>, ContextError> {
        let mut buffer = vec![0u8; self.buffer_size];
        let mut result = Vec::new();
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
                        break;
                    }
                    let (mut frame_size, mut frame) = self.read_frame(&self.read_buffer)?;
                    self.read_buffer.drain(0..frame_size);
                    let stream_id = self.handle_frame(&mut frame)?;
                    match self.streams.get_mut(&stream_id) {
                        Some(stream) => match stream.state {
                            StreamState::FillingData => {
                                if read_data_stream {
                                    result.push(stream.clone_reset_data());
                                }
                            }
                            StreamState::Completed => {
                                result.push(stream.clone());
                                self.streams.remove(&stream_id);
                            },
                            StreamState::Ended =>{
                                // self.streams.remove(stream.);
                            }
                            _ => {}
                        },
                        None => {}
                    };
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
        return Ok(result);
    }

    fn read_frame(&self, buf: &Vec<u8>) -> Result<(usize, Frame), ContextError> {
        let mut frame = <Frame as TryFrom<&[u8]>>::try_from(&buf)?;
        let len = <Frame as Len>::binary_len(&frame);
        Ok((len, frame))
    }

    fn handle_frame(&mut self, frame: &mut Frame) -> Result<u31, ContextError> {
        let stream = match self.streams.get_mut(&frame.stream_id) {
            Some(stream) => stream,
            None => {
                let mut stream = Http2Stream::new(frame.stream_id);
                self.streams.insert(frame.stream_id, stream);
                self.streams.get_mut(&frame.stream_id).unwrap()
            }
        };

        match &mut frame.payload {
            kparser::http2::Payload::Settings(settings_payload) => {
                for (id, value) in settings_payload.settings.iter() {
                    match id {
                        &SETTINGS_HEADER_TABLE_SIZE => {
                            self.hpack_context.resize(value.clone() as usize);
                        }
                        SETTINGS_ENABLE_PUSH => {
                            self.enable_push = (value.clone() != 0);
                        }
                        SETTINGS_MAX_CONCURRENT_STREAMS => self.max_streams = value.clone(),
                        SETTINGS_INITIAL_WINDOW_SIZE => {
                            if (frame.stream_id.to_u32() == 0) {
                                self.nax_window_size = value.clone() as u128;
                            } else {
                                stream.set_window_frame_size(value.clone())
                            }
                        }
                        SETTINGS_MAX_FRAME_SIZE => {
                            self.max_frame_size = value.clone();
                        }
                        SETTINGS_MAX_HEADER_LIST_SIZE => {
                            self.max_headers_len = value.clone();
                        }
                        _ => {}
                    }
                }
                stream.state = StreamState::Initiate;
            }
            kparser::http2::Payload::Data(data_payload) => {
                stream.write_data(data_payload);
                if frame.flags & DataPayloadFlag::END_STREAM == DataPayloadFlag::END_STREAM {
                    stream.state = StreamState::Completed;
                } else {
                    stream.state = StreamState::FillingData;
                }
            }
            kparser::http2::Payload::Headers(headers_payload) => {
                let (headers, headers_size) = headers_payload
                    .HeaderBlockFragment
                    .decode(&mut self.hpack_context)?;

                if (stream.get_headers_len() + headers_size as u32 > self.max_headers_len) && (self.max_headers_len != 0) {
                    return Err(ContextError::MaxHeaderLenExceeded);
                }

                stream.add_headers(headers, headers_size as u32);
                if frame.flags & HeadersPayloadFlag::END_STREAM == HeadersPayloadFlag::END_STREAM {
                    if frame.flags & HeadersPayloadFlag::END_HEADERS
                        == HeadersPayloadFlag::END_HEADERS
                    {
                        stream.state = StreamState::Completed;
                    } else {
                        stream.state = StreamState::FillingHeaders;
                    }
                } else {
                    stream.state = StreamState::FillingHeaders;
                }
            }
            kparser::http2::Payload::Priority(_) => {
                // The PRIORITY frame (type=0x02) is deprecated;
                // https://datatracker.ietf.org/doc/html/rfc9113#name-priority
            }
            kparser::http2::Payload::RstStream(rst_payload) => {}
            kparser::http2::Payload::PushPromise(_) => todo!(),
            kparser::http2::Payload::Ping(_) => todo!(),
            kparser::http2::Payload::GoAway(goaway_payload) => {
               return Err(ContextError::ClientDisconnected);
            },
            kparser::http2::Payload::WindowUpdate(window_update_payload) => {
                stream
                    .window_frame_size_increament(window_update_payload.WindowSizeIncrement);
            }
            kparser::http2::Payload::Continuation(continuation_payload) => {
                let (headers, headers_size) = continuation_payload
                    .HeaderBlockFragment
                    .decode(&mut self.hpack_context)?;

                if stream.get_headers_len() + headers_size as u32 > self.max_headers_len {
                    return Err(ContextError::MaxHeaderLenExceeded);
                }

                stream.add_headers(headers, headers_size as u32);
                if frame.flags & ContinuationPayloadFlag::END_HEADERS
                    == ContinuationPayloadFlag::END_HEADERS
                {
                    stream.state = StreamState::Completed;
                } else {
                    stream.state = StreamState::FillingHeaders;
                }
            }
        }
        Ok(stream.get_stream_id())
    }
}
