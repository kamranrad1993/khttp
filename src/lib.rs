pub static mut BUFFER_SIZE: usize = 4196;
pub mod http2;


#[cfg(test)]
mod tests {
    use http::Response;
    use http2::Http2Server;

    use super::*;

    #[test]
    fn it_works() {
        let mut server = Http2Server::new("127.0.0.1:8080").unwrap();
        server.listen(|token, request| -> Response<Vec<u8>> {
            Response::new(request.body().to_owned())
        }).unwrap();
    }
}
