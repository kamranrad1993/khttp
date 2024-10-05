use http::{request, Response};
use khttp::http2::Http2Server;





fn main() {
    let mut server = Http2Server::new("127.0.0.1:8080").unwrap();
    server.listen(|token, request| -> Response<Vec<u8>> {

        Response::new(request.body().to_owned())
    }).unwrap();
}