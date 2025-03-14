use alloc::string::String;
use core::time::Duration;

const DEFAULT_URL: &str = "http://localhost";
const DEFAULT_PORT: u16 = 8332; // the default RPC port for bitcoind.
const DEFAULT_TIMEOUT_SECONDS: u64 = 15;

#[derive(Clone, Debug)]
pub struct MinreqHttpTransport {
    /// URL of the RPC server.
    url: String,
    /// timeout only supports second granularity.
    timeout: Duration,
    /// The value of the `Authorization` HTTP header, i.e., a base64 encoding of 'user:password'.
    basic_auth: Option<String>,
}

impl Default for MinreqHttpTransport {
    fn default() -> Self {
        MinreqHttpTransport {
            url: append_port(DEFAULT_URL, DEFAULT_PORT),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
            basic_auth: None,
        }
    }
}

impl MinreqHttpTransport {
    /// Constructs a new [`MinreqHttpTransport`] with default parameters.
    pub fn new() -> Self { MinreqHttpTransport::default() }
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
}
