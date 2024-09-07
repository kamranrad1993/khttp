use std::io::Write;
use std::os::{fd::RawFd, unix::net::SocketAddr};
use std::time;

use kparser::{
    http2::{DataPayload, Hpack},
    u31::u31,
};
use mio::net::{TcpStream, UnixStream};

pub enum StreamState{
    None,
    Initiate,
    FillingHeaders,
    FillingData,
    Completed
}

pub struct Http2Stream {
    pub state: StreamState,
    pub stream_id: u31,
    max_window_frame_size: u128,
    data_stream: Option<UnixStream>,
    headers: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    headers_len: u32,
}

impl Http2Stream {
    pub fn new(stream_id: u31) -> Self {
        Self {
            state: StreamState::None,
            stream_id: stream_id,
            max_window_frame_size: 4096,
            headers: None,
            data_stream: None,
            headers_len: 0,
        }
    }

    pub fn set_window_frame_size(&mut self, value: u32) {
        self.max_window_frame_size = value as u128;
    }

    pub fn window_frame_size_increament(&mut self, value: u32) {
        self.max_window_frame_size += value as u128;
    }

    pub fn write_data_payload(&mut self, data: &mut DataPayload) {
        if self.data_stream.is_none() {
            let name = time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string();
            let stream = mio::net::UnixStream::connect(format!("/tmp/{}", name)).unwrap();
            self.data_stream = Some(stream);
        }

        self.data_stream.as_mut().unwrap().write(&data.data);
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

    pub fn clone(&mut self) -> Self{
        Self{
            state: self.state.clone(),
            stream_id: self.stream_id,
            max_window_frame_size: self.max_window_frame_size,
            data_stream: Some(self.data_stream.take().unwrap()),
            headers: Some(self.headers.take().unwrap()),
            headers_len: self.headers_len,
        }
    }
}


impl Clone for StreamState {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Initiate => Self::Initiate,
            Self::FillingHeaders => Self::FillingHeaders,
            Self::FillingData => Self::FillingData,
            Self::Completed => Self::Completed,
        }
    }
}