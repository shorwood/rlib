#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

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

struct WorkflowBuilder {
    state: u32,
    revision: u32,
}

impl WorkflowBuilder {
    fn new() -> Self {
        Self {
            state: 0,
            revision: 0,
        }
    }
    fn advance(self, _input: u32) -> Self {
        self
    }
    fn retry(self, _input: u32) -> Self {
        self
    }
    fn build(self) -> Request {
        Request {
            host: String::new(),
            port: 80,
        }
    }
}

struct CounterBuilder {
    minimum: u32,
    maximum: u32,
}

impl CounterBuilder {
    fn new() -> Self {
        Self {
            minimum: 0,
            maximum: 0,
        }
    }
    fn minimum(mut self, minimum: u32) -> Self {
        self.minimum = minimum;
        self
    }
    fn maximum(mut self, maximum: u32) -> Self {
        self.maximum = maximum;
        self
    }
    fn build(self) -> u32 {
        self.maximum - self.minimum
    }
}

fn main() {}
