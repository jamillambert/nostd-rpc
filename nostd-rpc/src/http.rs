use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{Medium, TunTapInterface};
use smoltcp::socket::tcp;
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

const DEFAULT_URL: &str = "http://localhost";
const DEFAULT_PORT: u16 = 8332; // the default RPC port for bitcoind.
const DEFAULT_TIMEOUT_SECONDS: u64 = 15;

#[derive(Clone, Debug)]
pub struct HttpRequest {
    /// URL of the RPC server.
    url: String,
    /// Port of the RPC server.
    port: u16,
    /// IPv4 address of the RPC server.
    ipv4: Ipv4Address,
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
            url: String::from(DEFAULT_URL),
            port: DEFAULT_PORT,
            ipv4: Ipv4Address::new(192, 168, 42, 1),
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
    pub fn new() -> Self {
        HttpRequest::default()
    }

    /// Sets the URL of the RPC server.
    pub fn url(mut self, url: &str) -> Self {
        self.url = String::from(url);
        self
    }

    /// Sets the port of the RPC server.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the ip the RPC server.
    pub fn ipv4(mut self, ip: [u8; 4]) -> Self {
        self.ipv4 = Ipv4Address::new(ip[0], ip[1], ip[2], ip[3]);
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

        if !self.url.starts_with('/') {
            request.push('/');
        }
        request.push_str(&self.url);

        request.push_str(" HTTP/1.1\r\n");
        request.push_str("Host: ");
        request.push_str("www.example.org"); //&self.ipv4.to_string()); // TODO: Fix this, doesnt work with an IP address for unknown reason
        request.push_str("\r\n");

        for header in &self.headers {
            request.push_str(header);
            request.push_str("\r\n");
        }

        request.push_str("Content-Length: ");
        request.push_str(&u16_to_string(self.body.len() as u16));
        request.push_str("\r\n");
        request.push_str("Connection: close\r\n");

        request.push_str("\r\n");
        request.push_str(&self.body);

        request
    }
}

pub fn send(ethernet_mac: [u8; 6], request: HttpRequest) -> Result<String, &'static str> {
    let mut device = TunTapInterface::new("tap0", Medium::Ethernet)
        .map_err(|_| "Failed to create TUN/TAP interface")?;
    let config = Config::new(EthernetAddress(ethernet_mac).into());

    let mut iface = Interface::new(config, &mut device, Instant::now());
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(IpAddress::v4(192, 168, 42, 1), 24)) // Local IP with subnet mask
            .map_err(|_| "Failed to update IP addresses")
            .unwrap()
    });
    iface
        .routes_mut()
        .add_default_ipv4_route(Ipv4Address::new(192, 168, 42, 100)) // Default gateway
        .map_err(|_| "Failed to add default route")?;

    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);
    let mut sockets = SocketSet::new(vec![]);
    let tcp_handle = sockets.add(tcp_socket);

    enum State {
        Connect,
        Request,
        Response,
    }
    let mut state = State::Connect;

    let mut response = String::from("");

    loop {
        let timestamp = Instant::now();
        let _ = iface.poll(timestamp, &mut device, &mut sockets);

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        let cx = iface.context();

        state = match state {
            State::Connect if !socket.is_active() => {
                socket
                    .connect(cx, (request.ipv4, 80), request.port)
                    .map_err(|_| "Failed to connect")?;
                response.push_str("Connected to server.\n");
                State::Request
            }
            State::Request if socket.may_send() => {
                let message = request.construct_http_request();
                socket
                    .send_slice(message.as_ref())
                    .map_err(|_| "Failed to send HTTP request")?;
                State::Response
            }
            State::Response if socket.can_recv() => {
                socket
                    .recv(|data| {
                        response.push_str(alloc::str::from_utf8(data).unwrap_or("(invalid utf8)"));
                        (data.len(), ())
                    })
                    .map_err(|_| "Failed to receive data")?;
                State::Response
            }
            State::Response if !socket.may_recv() => break,
            _ => state,
        };
    }

    Ok(response)
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
    fn test_get() {
        let request = HttpRequest::new()
            .url("/index.html")
            .port(80)
            .ipv4([104, 100, 168, 75])
            .method("GET")
            .timeout(Duration::from_secs(5));
        let ethernet_mac = [0x00, 0x15, 0x5d, 0xc7, 0xbf, 0x6d];

        let result = send(ethernet_mac, request).unwrap();
        assert!(result.len() > 0);
    }
}
