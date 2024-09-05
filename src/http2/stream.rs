use std::os::fd::RawFd;

use kparser::{http2::Hpack, u31::u31};

pub struct Http2Stream{
    stream_id: u31,
    window_frame_size: u128,
    data_stream: Option<RawFd>,
    headers: Option<Hpack>
}

impl Http2Stream {
    pub fn new(stream_id:u31) -> Self {
        Self{
            stream_id: stream_id,
            window_frame_size: 4096,
            data_stream: None, 
            headers:  None
        }
    }
}