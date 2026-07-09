use proc_macro::TokenStream;
use quote::{quote, quote_spanned};

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Error, FnArg, Ident, ItemFn, LitStr, Pat, Result, Token};

use crate::NATIVE_PREFIX;
use crate::REG_PREFIX;

struct NativeName {
    pub name: String,
    pub raw: bool,
}

impl Parse for NativeName {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = String::new();
        let mut raw = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;

            if ident == "name" {
                let _: Token![=] = input.parse()?;
                let native_name: LitStr = input.parse()?;

                name = native_name.value();
            } else if ident == "raw" {
                raw = true;
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Unexpected argument name. Currently supports only \"name\" and \"raw\".",
                ));
            }

            let _: Option<Token![,]> = input.parse()?;
        }

        Ok(NativeName { name, raw })
    }
}

/// Generates, next to a free function `fn foo(amx: &Amx, ...) -> AmxResult<T>`,
/// an `extern "C"` wrapper that builds the `Amx` handle and parses the
/// arguments, plus a `__samp_reg_foo()` constructor of `AMX_NATIVE_INFO`
/// for `initialize_plugin!`.
pub fn create_native(args: TokenStream, input: TokenStream) -> TokenStream {
    let native = parse_macro_input!(args as NativeName);
    let origin_fn = parse_macro_input!(input as ItemFn);

    if let Some(receiver) = origin_fn.sig.inputs.iter().find_map(|arg| match arg {
        FnArg::Receiver(receiver) => Some(receiver),
        FnArg::Typed(_) => None,
    }) {
        return Error::new(
            receiver.span(),
            "natives are free functions: there is no plugin object, keep state in a thread_local",
        )
        .to_compile_error()
        .into();
    }

    let vis = &origin_fn.vis;
    let origin_name = &origin_fn.sig.ident;
    let native_name = prepend(origin_name, NATIVE_PREFIX);
    let reg_name = prepend(origin_name, REG_PREFIX);
    let amx_name = &native.name;

    // Parameters after the first one (`amx: &Amx`).
    let param_idents: Vec<&Ident> = origin_fn
        .sig
        .inputs
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(typed) => match &*typed.pat {
                Pat::Ident(pat_ident) => Some(&pat_ident.ident),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect();

    let args_parsing: Vec<proc_macro2::TokenStream> = if native.raw {
        vec![]
    } else {
        param_idents
            .iter()
            .map(|ident| {
                quote_spanned! { ident.span() =>
                    let Some(#ident) = args.next_arg() else {
                        println!(
                            "{} error: couldn't parse the {:?} argument.",
                            #amx_name, stringify!(#ident),
                        );
                        return 0;
                    };
                }
            })
            .collect()
    };

    // `args` is moved into raw natives, mutated by parsing ones and unused
    // by parsing natives without parameters.
    let args_binding = if native.raw {
        quote!(args)
    } else if args_parsing.is_empty() {
        quote!(_args)
    } else {
        quote!(mut args)
    };

    let call_origin = if native.raw {
        quote!(#origin_name(&amx, args))
    } else {
        quote!(#origin_name(&amx, #(#param_idents),*))
    };

    let native_generated = quote! {
        #vis extern "C" fn #native_name(amx: *mut samp::raw::types::AMX, args: *mut i32) -> i32 {
            // An `Amx` is just the raw pointer plus the exports table, so
            // build it directly instead of consulting the AMX registry. This
            // also covers AMX instances that never went through `AmxLoad`
            // (e.g. when called through the GDK).
            let amx = samp::amx::Amx::new(amx, samp::plugin::amx_exports());
            let #args_binding = samp::args::Args::new(&amx, args);

            #(#args_parsing)*

            match #call_origin {
                Ok(retval) => samp::plugin::convert_return_value(retval),
                Err(err) => {
                    println!("{} error: {}", #amx_name, err);
                    0
                }
            }
        }
    };

    let reg_native = quote! {
        #vis fn #reg_name() -> samp::raw::types::AMX_NATIVE_INFO {
            samp::raw::types::AMX_NATIVE_INFO {
                name: std::ffi::CString::new(#amx_name).unwrap().into_raw(),
                func: #native_name,
            }
        }
    };

    let generated = quote! {
        #origin_fn
        #reg_native
        #native_generated
    };

    generated.into()
}

fn prepend(ident: &Ident, prefix: &str) -> Ident {
    Ident::new(&format!("{}{}", prefix, ident), ident.span())
}
