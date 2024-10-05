use std::fmt::Display;
use std::io::Write;
use std::os::{fd::RawFd, unix::net::SocketAddr};
use std::time;

use http::Request;
use kparser::{
    http2::{DataPayload, Hpack},
    u31::u31,
};
use mio::net::{TcpStream, UnixStream};

#[derive(Debug)]
pub enum StreamState {
    None,
    Initiate,
    Ping,
    FillingHeaders,
    FillingData,
    Completed,
}

#[derive(Debug)]
pub struct Http2Stream {
    pub state: StreamState,
    stream_id: u31,
    max_window_frame_size: u128,
    data: Option<Vec<u8>>,
    headers: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    headers_len: u32,
    pub ping_opaque: u64,
}

impl Http2Stream {
    pub fn new(stream_id: u31) -> Self {
        Self {
            state: StreamState::None,
            stream_id: stream_id,
            max_window_frame_size: 4096,
            headers: None,
            data: None,
            headers_len: 0,
            ping_opaque: 0,
        }
    }

    pub fn get_stream_id(&self) -> u31 {
        self.stream_id
    }

    pub fn set_window_frame_size(&mut self, value: u32) {
        self.max_window_frame_size = value as u128;
    }

    pub fn window_frame_size_increament(&mut self, value: u32) {
        self.max_window_frame_size += value as u128;
    }

    pub fn write_data(&mut self, data: &mut DataPayload) {
        if self.data.is_none() {
            self.data = Some(Vec::new());
        }

        self.data.as_mut().unwrap().write(&data.data);
    }

    pub fn read_data(&self) -> Option<Vec<u8>> {
        Some(self.data.as_ref().unwrap().clone())
    }

    pub fn get_headers_len(&self) -> u32 {
        return self.headers_len;
    }

    pub fn add_headers(&mut self, headers: Vec<(Vec<u8>, Vec<u8>)>, size: u32) {
        if self.headers.is_none() {
            self.headers = Some(Vec::new());
        }
        self.headers.as_mut().unwrap().extend(headers);
        self.headers_len += size;
    }

    pub fn clone(&self) -> Self {
        match &self.data {
            Some(data) => {
                let result = Self {
                    state: self.state.clone(),
                    stream_id: self.stream_id,
                    max_window_frame_size: self.max_window_frame_size,
                    data: Some(data.clone()),
                    headers: match &self.headers {
                        Some(headers) => Some(headers.clone()),
                        None => None,
                    },
                    headers_len: self.headers_len,
                    ping_opaque: self.ping_opaque,
                };
                result
            }
            None => {
                let result = Self {
                    state: self.state.clone(),
                    stream_id: self.stream_id,
                    max_window_frame_size: self.max_window_frame_size,
                    data: None,
                    headers: match &self.headers {
                        Some(headers) => Some(headers.clone()),
                        None => None,
                    },
                    headers_len: self.headers_len,
                    ping_opaque: self.ping_opaque,
                };
                result
            }
        }
    }

    pub fn clone_reset_data(&mut self) -> Self {
        match &mut self.data {
            Some(data) => {
                let result = Self {
                    state: self.state.clone(),
                    stream_id: self.stream_id,
                    max_window_frame_size: self.max_window_frame_size,
                    data: Some(data.clone()),
                    headers: match &self.headers {
                        Some(headers) => Some(headers.clone()),
                        None => None,
                    },
                    headers_len: self.headers_len,
                    ping_opaque: self.ping_opaque,
                };
                self.data.as_mut().unwrap().clear();
                result
            }
            None => {
                let result = Self {
                    state: self.state.clone(),
                    stream_id: self.stream_id,
                    max_window_frame_size: self.max_window_frame_size,
                    data: None,
                    headers: match &self.headers {
                        Some(headers) => Some(headers.clone()),
                        None => None,
                    },
                    headers_len: self.headers_len,
                    ping_opaque: self.ping_opaque,
                };
                result
            }
        }
    }
}

impl Clone for StreamState {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Initiate => Self::Initiate,
            Self::Ping => Self::Ping,
            Self::FillingHeaders => Self::FillingHeaders,
            Self::FillingData => Self::FillingData,
            Self::Completed => Self::Completed,
        }
    }
}

impl Display for Http2Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.write_str(format!("stream_id: {}", self.stream_id).as_str())?;
        write!(f, "stream_id: {}\n", self.stream_id)?;
        if self.headers.is_some() {
            // f.write_str("Headers:")?;
            write!(f, "Headers:\n")?;
            for (h, k) in self.headers.as_ref().unwrap() {
                let h = std::str::from_utf8(&h).unwrap_or("Unparsable Header");
                let k = std::str::from_utf8(&k).unwrap_or("Unparsable Header's Value");
                // f.write_str(format!("  {}:{}", h,k).as_str())?;
                write!(f, "  {}:{}\n", h, k)?;
            }
        }
        if self.data.is_some() {
            // f.write_str(format!("Data Length : {} ", self.data.as_ref().unwrap().len()).as_str())?;
            write!(f, "Data Length : {} ", self.data.as_ref().unwrap().len())?;
        }
        Ok(())
    }
}

impl Into<http::Request<Vec<u8>>> for Http2Stream {
    fn into(self) -> http::Request<Vec<u8>> {
        let mut builder = http::Request::builder();
        if self.headers.is_some() {
            for (key, value) in self.headers.unwrap() {
                match key.as_slice() {
                    [0x3A, 0x6D, 0x65, 0x74, 0x68, 0x6F, 0x64] => { // :method
                        builder = builder.header("method", value);
                    }
                    [0x3A, 0x70, 0x61, 0x74, 0x68] => { // :path
                        builder = builder.header("path", value);
                    }
                    [0x3A, 0x73, 0x63, 0x68, 0x65, 0x6D, 0x65] => { // :scheme
                        builder = builder.header("scheme", value);
                    }
                    [0x3A, 0x61, 0x75, 0x74, 0x68, 0x6F, 0x72, 0x69, 0x74, 0x79] => { // :authority
                        builder = builder.header("authority", value);
                    }
                    [0x3a, 0x73, 0x74, 0x61, 0x74, 0x75, 0x73] => { // :status
                        builder = builder.header("status", value);
                    }
                    _ => {
                        builder = builder.header(key, value);
                    }
                }
            }
        }

        if self.data.is_some() {
            return builder.body(self.data.unwrap()).unwrap();
        }

        return builder.body(vec![0u8; 0]).unwrap();
    }
}

impl Into<http::Response<Vec<u8>>> for Http2Stream {
    fn into(self) -> http::Response<Vec<u8>> {
        let mut builder = http::Response::builder();
        if self.headers.is_some() {
            for (key, value) in self.headers.unwrap() {
                match key.as_slice() {
                    [0x3A, 0x6D, 0x65, 0x74, 0x68, 0x6F, 0x64] => { // :method
                        builder = builder.header("method", value);
                    }
                    [0x3A, 0x70, 0x61, 0x74, 0x68] => { // :path
                        builder = builder.header("path", value);
                    }
                    [0x3A, 0x73, 0x63, 0x68, 0x65, 0x6D, 0x65] => { // :scheme
                        builder = builder.header("scheme", value);
                    }
                    [0x3A, 0x61, 0x75, 0x74, 0x68, 0x6F, 0x72, 0x69, 0x74, 0x79] => { // :authority
                        builder = builder.header("authority", value);
                    }
                    [0x3a, 0x73, 0x74, 0x61, 0x74, 0x75, 0x73] => { // :status
                        builder = builder.header("status", value);
                    }
                    _ => {
                        builder = builder.header(key, value);
                    }
                }
            }
        }

        if self.data.is_some() {
            return builder.body(self.data.unwrap()).unwrap();
        }

        return builder.body(vec![0u8; 0]).unwrap();
    }
}
