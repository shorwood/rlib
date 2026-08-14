// aux-build: external_macro.rs

#![warn(boolean_function_arguments)]
#![allow(dead_code, misordered_module_declarations)]

extern crate external_macro;

use external_macro::external_boolean_signature;

type Flag = bool;

fn render(minify: bool) {}

fn export(pretty: bool, include_metadata: Flag) {}

fn set_enabled(enabled: bool) {}

mod deceptive_setter {
    fn set_enabled(context: u32, enabled: bool) {}
}

fn set_enabled_indirectly(value: bool) {}

struct Settings;

impl Settings {
    fn set_visible(&mut self, visible: bool) {}

    fn set_enabled(&mut self, context: u32, enabled: bool) {}
}

fn choose(predicate: impl Fn(&str) -> bool) {}

fn tri_state(enabled: Option<bool>) {}

macro_rules! local_boolean_signature {
    () => {
        fn local_render(pretty: bool) {}
    };
}

local_boolean_signature!();
external_boolean_signature!();

trait Renderer {
    fn configure(minify: bool);
}

struct HtmlRenderer;

impl Renderer for HtmlRenderer {
    fn configure(_minify: bool) {}
}

fn main() {}
