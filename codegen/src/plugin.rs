use proc_macro::TokenStream;

use syn::parse_macro_input;
use syn::ItemStruct;

use quote::quote;

pub fn define_plugin(input: TokenStream) -> TokenStream {
    let plugin_struct = parse_macro_input!(input as ItemStruct);
    let plugin_name = &plugin_struct.ident;

    let extended = quote! {
        pub use self::__sdk_impl::log;

        pub mod __sdk_impl {
            use samp_sdk::prelude::*;
            use samp_sdk::types::RawAmx;
            use samp_sdk::internal::InternalData;

            use super::#plugin_name;

            use std::fmt::Display;

            pub static mut PLUGIN: Option<#plugin_name> = None;
            pub static mut INTERNAL: Option<InternalData> = None;

            pub fn log<T: Display>(message: T) {
                unsafe {
                    INTERNAL.as_ref()
                        .map(|internal| internal.log(message));
                }
            }

            impl SampPlugin for #plugin_name {
                type Plugin = #plugin_name;

                fn get() -> &'static mut Self::Plugin {
                    unsafe {
                        PLUGIN.as_mut().unwrap()
                    }
                }

                fn internal() -> &'static mut InternalData {
                    unsafe {
                        INTERNAL.as_mut().unwrap()
                    }
                }
            }

            #[no_mangle]
            pub unsafe extern "system" fn Load(data: *const *const u32) {
                PLUGIN = Some(#plugin_name::default());
                INTERNAL = Some(InternalData::new(data));

                #plugin_name::get().load();
            }

            #[no_mangle]
            pub unsafe extern "system" fn Unload() {
                #plugin_name::get().unload();
            }

            #[no_mangle]
            pub unsafe extern "system" fn AmxLoad(amx: *mut RawAmx) {
                let amx = Amx {
                    raw_ptr: amx,
                    internal: #plugin_name::internal(),
                };

                #plugin_name::get().amx_load(&amx);
            }

            #[no_mangle]
            pub unsafe extern "system" fn AmxUnload(amx: *mut RawAmx) {
                let amx = Amx {
                    raw_ptr: amx,
                    internal: #plugin_name::internal(),
                };

                #plugin_name::get().amx_unload(&amx);
            }

            #[no_mangle]
            pub extern "system" fn Supports() -> u32 {
                samp_sdk::consts::SUPPORTS_VERSION | samp_sdk::consts::SUPPORTS_AMX_NATIVES
            }
        }
    };

    extended.into()
}
