#![recursion_limit = "256"]
#![feature(proc_macro_span)]
#![feature(drain_filter)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use proc_macro::TokenStream;

mod natives;
mod plugin;

#[proc_macro_attribute]
pub fn contains_natives(_args: TokenStream, input: TokenStream) -> TokenStream {
    natives::create_extern_natives(input)
}

#[proc_macro_derive(SampPlugin)]
pub fn derive_samp_plugin(input: TokenStream) -> TokenStream {
    plugin::define_plugin(input)
}
