#![allow(dead_code, misordered_module_declarations, unknown_lints)]

struct Request {
    host: String,
    port: u16,
}

struct RequestBuilder {
    host: String,
    port: u16,
}

impl RequestBuilder {
    fn new() -> Self {
        Self {
            host: String::new(),
            port: 80,
        }
    }

    fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    fn build(self) -> Request {
        Request {
            host: self.host,
            port: self.port,
        }
    }
}

struct CheckedBuilder;

impl CheckedBuilder {
    fn new() -> Self {
        Self
    }
    fn value(self, _value: u32) -> Self {
        self
    }
    fn build(self) -> Result<Request, String> {
        Err(String::new())
    }
}

fn main() {}
