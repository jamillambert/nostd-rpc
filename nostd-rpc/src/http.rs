use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpAddress, Ipv4Address, IpCidr};
use smoltcp::phy::{TunTapInterface, Medium};

const DEFAULT_URL: &str = "http://localhost";
const DEFAULT_PORT: u16 = 8332; // the default RPC port for bitcoind.
const DEFAULT_TIMEOUT_SECONDS: u64 = 15;

#[derive(Clone, Debug)]
pub struct HttpRequest {
    /// URL of the RPC server.
    url: String,
    /// HTTP method, e.g., "POST".
    method: String,
    /// HTTP headers.
    headers: Vec<String>,
    /// Body of the HTTP request.
    body: String,
    /// timeout only supports second granularity.
    timeout: Duration,
    /// The value of the `Authorization` HTTP header, i.e., a base64 encoding of 'user:password'.
    basic_auth: Option<String>,
}

impl Default for HttpRequest {
    fn default() -> Self {
        HttpRequest {
            url: append_port(DEFAULT_URL, DEFAULT_PORT),
            method: String::from("POST"),
            headers: Vec::new(),
            body: String::new(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
            basic_auth: None,
        }
    }
}

impl HttpRequest {
    /// Constructs a new [`MinreqHttpTransport`] with default parameters.
    pub fn new() -> Self { HttpRequest::default() }

    /// Sets the URL of the RPC server.
    pub fn url(mut self, url: &str) -> Self {
        self.url = append_port(url, DEFAULT_PORT);
        self
    }

    /// Sets the HTTP method.
    pub fn method(mut self, method: &str) -> Self {
        self.method = String::from(method);
        self
    }

    /// Adds an HTTP header.
    pub fn header(mut self, header: &str) -> Self {
        self.headers.push(String::from(header));
        self
    }

    /// Sets the body of the HTTP request.
    pub fn body(mut self, body: &str) -> Self {
        self.body = String::from(body);
        self
    }

    /// Sets the timeout for the HTTP request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Manually construct the HTTP request as a string.
    pub fn construct_http_request(&self) -> String {
        let mut request = String::new();
        request.push_str(&self.method);
        request.push(' ');
        request.push_str(&self.url);
        request.push_str(" HTTP/1.1\r\n");
        for header in &self.headers {
            request.push_str(header);
            request.push_str("\r\n");
        }
        if let Some(basic_auth) = &self.basic_auth {
            request.push_str("Authorization: Basic ");
            request.push_str(basic_auth);
            request.push_str("\r\n");
        }
        request.push_str("Content-Length: ");
        request.push_str(&u16_to_string(self.body.len() as u16));
        request.push_str("\r\n\r\n");
        request.push_str(&self.body);
        request
    }
}

pub fn send(
    ip: [u8; 4],
    port: u16,
    _payload: String,
) -> String {
    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);
    let mut device = TunTapInterface::new("tap0", Medium::Ethernet).unwrap();
    let mut config = Config::new(EthernetAddress([0x00, 0x15, 0x5d, 0xc7, 0xbf, 0x6d]).into()); //00:15:5d:c7:bf:6d
    config.random_seed = 0; // Use a fixed seed for testing purposes.

    let mut iface = Interface::new(config, &mut device, Instant::now());
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(IpAddress::v4(172, 28, 24, 156), 20)) // Local IP with subnet mask
            .unwrap();
    });
    iface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::new(172, 28, 16, 1)) // Default gateway
        .unwrap();

    let mut sockets = SocketSet::new(vec![]);
    let tcp_handle = sockets.add(tcp_socket);  let ipv4 = Ipv4Address::new(ip[0], ip[1], ip[2], ip[3]);

    enum State {
        Connect,
        Request,
        Response,
    }
    let mut state = State::Connect;

    let mut response = String::new();

    for _ in 0..100 {
        let timestamp = Instant::now();
        iface.poll(timestamp, &mut device, &mut sockets);

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        let cx = iface.context();

        state = match state {
            State::Connect if !socket.is_active() => {
                let _ = socket
                    .connect(cx, (ipv4, 80), port);
                State::Request
            }
            State::Request if socket.may_send() => {
                let http_get = "GET www.example.org HTTP/1.1\r\n";
                socket.send_slice(http_get.as_ref()).expect("cannot send");
                let http_host = "Host: http://www.example.org/\r\n";
                socket.send_slice(http_host.as_ref()).expect("cannot send");
                socket
                    .send_slice(b"Connection: close\r\n")
                    .expect("cannot send");
                socket.send_slice(b"\r\n").expect("cannot send");
                State::Response
            }
            State::Response if socket.can_recv() => {
                socket
                    .recv(|data| {
                        // Append the received data to the response buffer
                        response.push_str(alloc::str::from_utf8(data).expect("Invalid UTF-8 sequence"));
                        (data.len(), ())
                    })
                    .unwrap();
                State::Response
            }
            State::Response if !socket.may_recv() => break,
            _ => state,
        };

        let start_time = Instant::now();
        let mut waiting = 0;
        // wait for 0.1 seconds
        while waiting < 100 {
            let current_time = Instant::now();
            let duration = current_time - start_time;
            waiting = duration.total_millis();
        }
    }

    response
}

fn append_port(url: &str, port: u16) -> String {
    // Append the port to the URL and return the string in no-std.
    let mut url = String::from(url);
    url.push(':');
    url.push_str(&u16_to_string(port));
    url
}

fn u16_to_string(value: u16) -> String {
    if value == 0 {
        return String::from("0");
    }
    let mut buffer = [0u8; 5];
    let mut i = buffer.len();
    let mut value = value;
    while value > 0 {
        i -= 1;
        buffer[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    String::from_utf8_lossy(&buffer[i..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_port() {
        assert_eq!(append_port("http://localhost", 8332), "http://localhost:8332");
    }

    #[test]
    fn test_u16_to_string() {
        assert_eq!(u16_to_string(8332), "8332");
    }

    #[test]
    fn test_send() {
        let ip = [81, 130, 109, 40];
        let port = 1234;
        let payload = String::from("Test payload");

        let result = send(ip, port, payload);
        assert_eq!(result, "expected result");
    }
}
