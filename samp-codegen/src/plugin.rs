use proc_macro::TokenStream;
use quote::quote;

use syn::parse::{Parse, ParseStream};
use syn::{bracketed, parse_macro_input, Block, Error, Ident, Path, Result, Stmt, Token};

use crate::REG_PREFIX;

/// ```ignore
/// initialize_plugin!(
///     natives: [my_native, other_native],   // optional
///     on_unload: path::to::fn,              // optional, fn()
///     on_amx_load: path::to::fn,            // optional, fn(&Amx)
///     on_amx_unload: path::to::fn,          // optional, fn(&Amx)
///     process_tick: path::to::fn,           // optional, fn(); enables PROCESS_TICK support
///     {
///         // setup block, runs in Load() — set up logging etc.
///     }
/// );
/// ```
struct InitPlugin {
    natives_list: Vec<Path>,
    on_unload: Option<Path>,
    on_amx_load: Option<Path>,
    on_amx_unload: Option<Path>,
    process_tick: Option<Path>,
    block: Vec<Stmt>,
}

impl Parse for InitPlugin {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut plugin = InitPlugin {
            natives_list: vec![],
            on_unload: None,
            on_amx_load: None,
            on_amx_unload: None,
            process_tick: None,
            block: vec![],
        };

        loop {
            // Only commit to `key:` parsing when it's one of our keywords,
            // so the setup block can still start with arbitrary statements.
            let fork = input.fork();
            let Ok(ident) = fork.parse::<Ident>() else { break };
            if !fork.peek(Token![:]) {
                break;
            }
            let key = ident.to_string();
            if !matches!(
                key.as_str(),
                "natives" | "on_unload" | "on_amx_load" | "on_amx_unload" | "process_tick"
            ) {
                break;
            }

            let ident: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;

            match key.as_str() {
                "natives" => {
                    let content;
                    let _ = bracketed!(content in input);
                    let natives = content.parse_terminated(Path::parse, Token![,])?;
                    plugin.natives_list = natives.into_iter().collect();
                }
                _ => {
                    let path: Path = input.parse()?;
                    let slot = match key.as_str() {
                        "on_unload" => &mut plugin.on_unload,
                        "on_amx_load" => &mut plugin.on_amx_load,
                        "on_amx_unload" => &mut plugin.on_amx_unload,
                        _ => &mut plugin.process_tick,
                    };
                    if slot.replace(path).is_some() {
                        return Err(Error::new(ident.span(), format!("duplicate `{}` hook", key)));
                    }
                }
            }

            let _: Option<Token![,]> = input.parse()?;
        }

        plugin.block = input.call(Block::parse_within)?;
        Ok(plugin)
    }
}

pub fn create_plugin(input: TokenStream) -> TokenStream {
    let plugin = parse_macro_input!(input as InitPlugin);
    let block = &plugin.block;

    let natives: Vec<proc_macro2::TokenStream> = plugin
        .natives_list
        .iter()
        .cloned()
        .map(|mut path| {
            if let Some(last_part) = path.segments.last_mut() {
                last_part.ident = Ident::new(
                    &format!("{}{}", REG_PREFIX, last_part.ident),
                    last_part.ident.span(),
                );
            }
            quote!(#path())
        })
        .collect();

    let has_process_tick = plugin.process_tick.is_some();

    let unload_body = plugin.on_unload.map(|path| quote!(#path();));

    let amx_load_body = match plugin.on_amx_load {
        Some(path) => quote! {
            samp::interlayer::amx_load(amx, &natives);
            if let Some(amx) = std::ptr::NonNull::new(amx) {
                samp::amx::enter(amx, |amx| #path(amx));
            }
        },
        None => quote! {
            samp::interlayer::amx_load(amx, &natives);
        },
    };

    // The hook runs before unregistration so `amx.handle()` still resolves.
    let amx_unload_body = match plugin.on_amx_unload {
        Some(path) => quote! {
            if let Some(amx) = std::ptr::NonNull::new(amx) {
                samp::amx::enter(amx, |amx| #path(amx));
            }
            samp::interlayer::amx_unload(amx);
        },
        None => quote! {
            samp::interlayer::amx_unload(amx);
        },
    };

    let process_tick_export = plugin.process_tick.map(|path| {
        quote! {
            #[no_mangle]
            pub extern "system" fn ProcessTick() {
                #path();
            }
        }
    });

    let generated = quote! {
        #[no_mangle]
        pub extern "system" fn Supports() -> u32 {
            samp::interlayer::supports(#has_process_tick)
        }

        #[no_mangle]
        pub extern "system" fn Load(server_data: *const usize) -> i32 {
            samp::interlayer::load(server_data);

            {
                #(#block)*
            }

            samp::plugin::finish_setup();
            return 1;
        }

        #[no_mangle]
        pub extern "system" fn Unload() {
            #unload_body
        }

        #[no_mangle]
        pub extern "system" fn AmxLoad(amx: *mut samp::raw::types::AMX) {
            let natives = vec![#(#natives),*];

            #amx_load_body
        }

        #[no_mangle]
        pub extern "system" fn AmxUnload(amx: *mut samp::raw::types::AMX) {
            #amx_unload_body
        }

        #process_tick_export
    };

    generated.into()
}
