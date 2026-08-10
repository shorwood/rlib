#![warn(ad_hoc_iterators)]
#![allow(dead_code, misordered_module_declarations, missing_section_dividers)]

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
