#[cfg(test)]
mod tests {
    use nostd_rpc::http;
    use smoltcp::time::Duration;

    #[test]
    fn get() {
        let request = http::HttpRequest::new()
            .ipv4([104, 100, 168, 75]) // IP address for www.example.com (may change)
            .port(80)
            .host("www.example.com")
            .url("/index.html")
            .method("GET")
            .timeout(Duration::from_secs(5));
        let ethernet_mac = [0x05, 0x2d, 0x1e, 0xef, 0x5c, 0x45];
        let result = http::send(ethernet_mac, request).unwrap();
        let parsed = http::decode_html(&result);

        let expected_response_start = "Connected to server.\nHTTP/1.1 200 OK";
        assert!(
            result.starts_with(expected_response_start),
            "Unexpected response: \n\n{}",
            parsed
        );
    }

    #[test]
    fn post() {
        let request = http::HttpRequest::new()
            .ipv4([54, 152, 142, 77]) // IP address for httpbin.org (may change)
            .port(80)
            .url("/post")
            .host("httpbin.org")
            .method("POST")
            .header("Content-Type: application/json")
            .body("{\"key1\": \"value1\", \"key2\": \"value2\"}")
            .timeout(Duration::from_secs(5));
        let ethernet_mac = [0x05, 0x2d, 0x1e, 0xef, 0x5c, 0x45];
        let result = http::send(ethernet_mac, request).unwrap();
        let parsed = http::decode_html(&result);

        let expected_response_start = "Connected to server.\nHTTP/1.1 200 OK";
        assert!(
            result.starts_with(expected_response_start),
            "Unexpected response: \n\n{}",
            parsed
        );
    }
}
