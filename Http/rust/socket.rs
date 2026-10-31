//! HTTP/1.1 over `std::net`, with TLS from [`crate::tls`].
use crate::tls::{self, TlsStream};
use crate::{Capabilities, ClientConfig, HttpError, Request, Response, Url, io_error};
use crate::{redirect, wire};
use std::io::{BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

pub fn capabilities() -> Capabilities {
    match tls::available() {
        Ok(()) => Capabilities {
            http: true,
            https: true,
            limitation: None,
        },
        Err(_) => Capabilities {
            http: true,
            https: false,
            limitation: Some("https needs the system OpenSSL (libssl), which could not be loaded"),
        },
    }
}

pub fn send(config: &ClientConfig, request: Request, url: Url) -> Result<Response, HttpError> {
    let Request {
        mut method,
        mut headers,
        mut body,
        ..
    } = request;
    let mut url = url;
    let mut redirects = 0;
    loop {
        let (status, response_headers, reader) = exchange(config, &method, &url, &headers, &body)?;
        let follow = redirect::Follow {
            method: &mut method,
            headers: &mut headers,
            body: &mut body,
            redirects: &mut redirects,
        };
        let location = response_headers.get("location");
        match redirect::next(status, location, &url, config.max_redirects, follow)? {
            Some(next) => url = next,
            None => {
                let head = wire::ResponseHead {
                    status,
                    headers: response_headers,
                };
                let body = wire::body_reader(&method, &head, reader)?;
                return Ok(Response::new(status, head.headers, url.to_string(), body));
            }
        }
    }
}

type Exchange = (u16, crate::Headers, BufReader<Box<dyn Read + Send>>);

fn exchange(
    config: &ClientConfig,
    method: &str,
    url: &Url,
    headers: &crate::Headers,
    body: &[u8],
) -> Result<Exchange, HttpError> {
    let socket = connect(url, config)?;
    let mut stream: Box<dyn Stream> = if url.secure {
        Box::new(TlsStream::connect(socket, &url.host, url.is_ip_literal())?)
    } else {
        Box::new(socket)
    };
    let head = wire::request_head(method, url, headers, body.len(), &config.user_agent);
    stream
        .write_all(&head)
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush())
        .map_err(io_error)?;
    let mut reader = BufReader::new(stream.into_read());
    let head = wire::read_head(&mut reader)?;
    Ok((head.status, head.headers, reader))
}

trait Stream: Read + Write + Send {
    fn into_read(self: Box<Self>) -> Box<dyn Read + Send>;
}

impl<T: Read + Write + Send + 'static> Stream for T {
    fn into_read(self: Box<Self>) -> Box<dyn Read + Send> {
        self
    }
}

fn connect(url: &Url, config: &ClientConfig) -> Result<TcpStream, HttpError> {
    let addresses = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|error| HttpError::Network(format!("cannot resolve {}: {error}", url.host)))?;
    let timeout = config.timeout.max(std::time::Duration::from_millis(1));
    let mut last = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(socket) => {
                socket
                    .set_read_timeout(Some(timeout))
                    .and_then(|()| socket.set_write_timeout(Some(timeout)))
                    .and_then(|()| socket.set_nodelay(true))
                    .map_err(io_error)?;
                return Ok(socket);
            }
            Err(error) => last = Some(error),
        }
    }
    Err(match last {
        Some(error) => io_error(error),
        None => HttpError::Network(format!("{} resolved no address", url.host)),
    })
}
