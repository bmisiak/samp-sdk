use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use quote::{quote, quote_spanned};

use syn::fold::Fold;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::{ImplItem, ItemImpl, MethodSig, Path, Type, FnArg};

struct NativesVisitor {
    plugin_name: Path,
    natives: Vec<MethodSig>,
}

impl NativesVisitor {
    fn new(ty: &Type) -> NativesVisitor {
        if let Type::Path(type_path) = ty {
            let path = type_path.path.clone();

            NativesVisitor {
                plugin_name: path,
                natives: Vec::new(),
            }
        } else {
            ty.span()
                .unstable()
                .error("#[contains_natives] works only for simple types.")
                .emit();

            std::process::abort();
        }
    }

    fn generate_extern_natives(&self) -> TokenStream2 {
        let plugin_name = &self.plugin_name;
        let natives = self.natives
            .iter()
            .map(|native| {
                let method_name = &native.ident;
                let mut argument_idx = 0isize;

                let arg_parsing = native.decl.inputs.iter().skip(2).map(|input| {
                    if let FnArg::Captured(arg) = input {
                        let pat = &arg.pat;
                        let ty = &arg.ty;

                        argument_idx += 1;

                        quote_spanned!(arg.span() => let #pat: #ty = <#ty as AmxValueDecode>::decode(&__amx, args.offset(#argument_idx).read(), None).unwrap();)
                    } else {
                        quote!()
                    }
                }).collect::<Vec<_>>();

                let arg_list = native.decl.inputs.iter().skip(2).map(|input| {
                    if let FnArg::Captured(arg) = input {
                        let pat = &arg.pat;

                        quote_spanned!(arg.span() => #pat)
                    } else {
                        quote!()
                    }
                }).collect::<Vec<_>>();

                let return_value = quote_spanned!{ native.span() =>
                    let result: AmxResult<_> = __plugin.#method_name(&__amx, #(#arg_list),*);
                    
                    match result {
                        Ok(value) => std::mem::transmute(value),
                        Err(error) => {
                            let error_msg = format!("samp-sdk error: {} (in {})", error, stringify!(#method_name));
                            #plugin_name::internal().log(error_msg);

                            return 0;
                        },
                    }
                };

                quote! {
                    #[no_mangle]
                    pub unsafe extern "C" fn #method_name(amx: *mut RawAmx, args: *mut usize) -> i32 {
                        let __plugin = #plugin_name::get();
                        let __amx = Amx {
                            raw_ptr: amx,
                            internal: #plugin_name::internal(),
                        };
                        
                        #(#arg_parsing)*
                        #return_value
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            pub mod __native_impl {
                use samp_sdk::prelude::*;
                use samp_sdk::types::RawAmx;
                use samp_sdk::arguments::AmxValueDecode;

                use super::#plugin_name;

                #(#natives)*
            }
        }
    }
}

impl Fold for NativesVisitor {
    fn fold_impl_item(&mut self, mut impl_item: ImplItem) -> ImplItem {
        if let ImplItem::Method(ref mut method) = impl_item {
            let removed_items = method
                .attrs
                .drain_filter(|attr| attr.path.is_ident("native"))
                .collect::<Vec<_>>();

            if removed_items.len() == 1 {
                self.natives.push(method.sig.clone());
            }
        }

        return impl_item;
    }
}

pub fn create_extern_natives(input: TokenStream) -> TokenStream {
    let item_impl = parse_macro_input!(input as ItemImpl);

    let mut visitor = NativesVisitor::new(&item_impl.self_ty);

    let fixed_impl = visitor.fold_item_impl(item_impl);
    let native_mod = visitor.generate_extern_natives();

    let extended = quote! {
        #fixed_impl
        #native_mod
    };

    extended.into()
}
