#![warn(rlib::ad_hoc_iterators)]
#![allow(dead_code, rlib::misordered_module_declarations, rlib::missing_section_dividers)]

#[derive(Clone)]
struct Token;

struct TokenStream {
    tokens: Vec<Token>,
    cursor: usize,
}

impl TokenStream {
    fn next_token(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }
}

struct DelegatingStream {
    inner: std::vec::IntoIter<Token>,
}

impl DelegatingStream {
    fn next_token(&mut self) -> Option<Token> {
        self.inner.next()
    }
}

struct EarlyDelegatingStream {
    inner: std::vec::IntoIter<Token>,
}

impl EarlyDelegatingStream {
    fn take_token(&mut self) -> Option<Token> {
        return self.inner.next();
    }
}

struct DiscardedDelegation {
    inner: std::vec::IntoIter<Token>,
}

impl DiscardedDelegation {
    // False-positive boundary: discarded traversal does not supply this method's result.
    fn next_token(&mut self) -> Option<Token> {
        let _ = self.inner.next();
        None
    }
}

struct DormantDelegation {
    inner: std::vec::IntoIter<Token>,
}

impl DormantDelegation {
    // False-positive boundary: a closure body is not executed by the enclosing method.
    fn next_token(&mut self) -> Option<Token> {
        let _later = || self.inner.next();
        None
    }
}

struct UnrelatedMutation {
    advances: usize,
}

impl UnrelatedMutation {
    // False-positive boundary: incidental receiver mutation does not make a constant an item.
    fn next_token(&mut self) -> Option<Token> {
        self.advances += 1;
        Some(Token)
    }
}

struct TelemetryMutation {
    tokens: Vec<Token>,
    observations: usize,
}

impl TelemetryMutation {
    // False-positive boundary: telemetry mutation does not advance the returned sequence.
    fn next_token(&mut self) -> Option<Token> {
        self.observations += 1;
        self.tokens.first().cloned()
    }
}

struct Queue {
    values: std::collections::VecDeque<Token>,
}

impl Queue {
    fn next_token(&mut self) -> Option<Token> {
        self.values.pop_front()
    }
}

struct Reusable {
    values: Vec<Token>,
    cursor: usize,
}

impl Reusable {
    fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.values.iter()
    }

    fn next_token(&mut self) -> Option<Token> {
        let token = self.values.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }
}

struct Competing {
    values: Vec<Token>,
    cursor: usize,
}

impl Competing {
    fn next_token(&mut self) -> Option<Token> {
        let token = self.values.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }

    fn take_next_token(&mut self) -> Option<Token> {
        let token = self.values.get(self.cursor)?.clone();
        self.cursor += 1;
        Some(token)
    }
}

fn main() {}
