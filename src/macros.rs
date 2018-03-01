/*!
Some useful macros to easy access and define natives.

Most of them hide raw C bindings and exports to make code easier to understand.
*/

/// Clear macros that makes a new `Vec<AMX_NATIVE_INFO>`.
///
/// # Examples
///
/// ```
/// let natives = natives![
///    { "ShowSomething", show_something },
///    { "WhereIsPlayer", where_is_player }
/// ];
/// amx.register(natives);
/// ```
#[macro_export]
macro_rules! natives {
    [ $( { $name:expr, $func:ident } ),* ] => {
        {
            let natives = vec![
                $(
                    $crate::types::AMX_NATIVE_INFO {
                        name: ::std::ffi::CString::new($name).unwrap().into_raw(),
                        func: $func,
                    }
                ),*
            ];

            natives
        }
    };

    [ $( $name:expr => $func:ident ),* ] => {
        {
            let natives = vec![
                $(
                    $crate::types::AMX_NATIVE_INFO {
                        name: ::std::ffi::CString::new($name).unwrap().into_raw(),
                        func: $func,
                    }
                ),*
            ];

            natives
        }
    };
}

/// Hides ugly C code from your eyes.
/// 
/// Generates raw extern C functions and makes call to your own static methods. 
///
/// # Examples
///
/// ```
/// struct MyPlugin;
/// 
/// impl MyPlugin {
///     fn load(&self) -> bool {
///         amx_log!("My plugin is loaded!");
///         return true;
///     }
///     
///     fn unload(&self);
///     fn amx_load(&self, amx: AMX) -> Cell;
///     fn amx_unload(&self, amx: AMX) -> Cell;
/// }
///
/// impl Default for MyPlugin {
///     fn default() -> MyPlugin {
///         MyPlugin{}
///     }
/// }
/// 
/// new_plugin!(MyPlugin) 
/// ```
/// To make a plugin with `ProccessTick` support use this:
/// ```
/// impl MyPlugin {
///     fn process_tick(&self);
/// }
///
/// new_plugin!(MyPlugin with process_tick);
/// ```
#[macro_export]
macro_rules! new_plugin {
    (@internal $name:ident) => {
        lazy_static! {
            static ref ___PLUGIN: ::std::sync::Mutex<$name> = ::std::sync::Mutex::new($name::default());
        }

        #[no_mangle]
        pub unsafe extern "C" fn Load(data: *const *const u32) -> bool {
            let mut log = $crate::data::logprintf.lock().unwrap();

            *log = *(data as *const $crate::types::Logprintf_t);
            $crate::data::amx_functions = std::ptr::read(data.offset($crate::consts::PLUGIN_DATA_AMX_EXPORTS as isize) as *const *const u32);

            drop(log);
            ___PLUGIN.lock().unwrap().load()
        }

        #[no_mangle]
        pub extern "C" fn Unload() {
            ___PLUGIN.lock().unwrap().unload();
        }

        #[no_mangle]
        pub extern "C" fn AmxLoad(amx: *mut $crate::types::AMX) -> u32 {
            ___PLUGIN.lock().unwrap().amx_load($crate::amx::AMX::new(amx))
        }

        #[no_mangle]
        pub extern "C" fn AmxUnload(amx: *mut $crate::types::AMX) -> u32 {
            ___PLUGIN.lock().unwrap().amx_unload($crate::amx::AMX::new(amx))
        }
    };

    ($name:ident) => {
        new_plugin!(@internal $name);

        #[no_mangle]
        pub extern "C" fn Supports() -> u32 {
            $crate::consts::SUPPORTS_VERSION | $crate::consts::SUPPORTS_AMX_NATIVES
        }
    };

    ($name:ident with process_tick) => {
        new_plugin!(@internal $name);

        #[no_mangle]
        pub extern "C" fn ProcessTick() {
            ___PLUGIN.lock().unwrap().process_tick();
        }

        #[no_mangle]
        pub extern "C" fn Supports() -> u32 {
            $crate::consts::SUPPORTS_PROCESS_TICK | $crate::consts::SUPPORTS_VERSION | $crate::consts::SUPPORTS_AMX_NATIVES
        }
    }
}

#[macro_export]
macro_rules! log {
    ($( $arg:tt )* ) => {
        {
            let printf = $crate::data::logprintf.lock().unwrap();
            let c_text = ::std::ffi::CString::new(format!($( $arg )*)).unwrap();
            printf(c_text.as_ptr());
        }
    }
}

/// Define native and hide raw C export functions.
///
/// # Examples
/// Define a native with raw params (`*mut Cell`).
/// ```
/// // native: WithRawParams(&arg1, arg2, arg3);
/// define_native!(with_raw_params as raw);
///
/// fn with_raw_params(&self, amx: AMX, args: *mut Cell) -> AmxResult<Cell>;
/// ```
///
/// Define a native without arguments.
/// ```
/// // native: WithoutArguments();
/// define_native!(without_arguments);
///
/// fn without_arguments(&self, amx: AMX) -> AmxResult<Cell>;
/// ```
/// 
/// Define a native with converted arguments.
/// ```
/// // native: SomeFunction(&int_val, float_val);
/// define_native!(some_function, int_val: ref i32, float_val: f32);
///
/// fn some_function(&self, amx: AMX, int_val: &mut i32, float_val: f32) -> AmxResult<Cell>;
/// ```
#[macro_export]
macro_rules! define_native {
    ($name:ident as raw) => {
        pub extern "C" fn $name(amx: *mut $crate::types::AMX, params: *mut $crate::types::Cell) -> $crate::types::Cell {
            let mut amx = $crate::amx::AMX::new(amx);
            match super::___PLUGIN.lock().unwrap().$name(&mut amx, params) {
                Ok(res) => return res,
                Err(err) => {
                    amx.raise_error(err).unwrap();
                    return 0;
                },
            };
        }
    };

    ($name:ident) => {
        pub extern "C" fn $name(amx: *mut $crate::types::AMX, _: *mut $crate::types::Cell) -> $crate::types::Cell {
            let mut amx = $crate::amx::AMX::new(amx);
            match super::___PLUGIN.lock().unwrap().$name(&mut amx) {
                Ok(res) => return res,
                Err(err) => {
                    amx.raise_error(err).unwrap();
                    return 0;
                },
            };
        }
    };

    ($name:ident, $( $arg:ident : $( $data:ident )+ ),* ) => {
        pub extern "C" fn $name(amx: *mut $crate::types::AMX, params: *mut $crate::types::Cell) -> $crate::types::Cell {
            let mut amx = $crate::amx::AMX::new(amx);
            expand_args!(amx, params, $( $arg : $( $data )+ ),* );
            
            let retval = super::___PLUGIN.lock().unwrap().$name(&mut amx, $( 
                    ___internal_expand_arguments!( 
                        $arg: $( $data )+ 
                    )
                ),* 
            );

            ___internal_forget!( $( $arg : $( $data )+ ),* );

            match retval {
                Ok(res) => return res,
                Err(err) => {
                    amx.raise_error(err).unwrap();
                    return 0;
                },
            };
        }
    }
}

#[macro_export]
macro_rules! ___internal_forget {
    (
        $arg:ident : ref $type:ty
    ) => {
        ::std::mem::forget($arg);
    };

    (
        $arg:ident : $type:ty
    ) => ();

    (
        $arg:ident : ref $type:ty,
        $( $tail_arg:ident : $( $tail_data:ident )+ ),*
    ) => {
        ___internal_forget!( $arg : ref $type );
        ___internal_forget!( $( $tail_arg : $( $tail_data )+ ),* );
    };

    (
        $arg:ident : $type:ty,
        $( $tail_arg:ident : $( $tail_data:ident )+ ),*
    ) => {
        ___internal_forget!( $( $tail_arg : $( $tail_data )+ ),* );
    };
}

#[macro_export]
macro_rules! ___internal_expand_arguments {
    (
        $arg:ident : ref $type:ty
    ) => {
        $arg.as_mut()
    };

    (
        $arg:ident : $type:ty
    ) => {
        $arg
    };
}

#[macro_export]
macro_rules! expand_args {
    // A string.
    (
        @
        $amx:ident,
        $parser:ident,

        $arg:ident : String,
    ) => {
        unimplemented!();
    };

    // TODO: A reference to a string.
    (
        @
        $amx:ident,
        $parser: ident,

        $arg:ident : ref String,
    ) => {
        unimplemented!();
    };

    // A reference to an primitive value.
    (
        @
        $amx:ident,
        $parser:ident,
        
        $arg:ident : ref $type:ty
    ) => {
        let mut $arg: Box<$type> = unsafe {
            let ptr = $parser.next();
            match $amx.get_address(::std::ptr::read(ptr as *const $crate::types::Cell)) {
                Ok(res) => res,
                Err(err) => {
                    $amx.raise_error(err).unwrap();
                    return 0;
                },
            }
        };
    };

    // An primitive value.
    (
        @
        $amx:ident,
        $parser:ident,

        $arg:ident : $type:ty
    ) => {
        let $arg: $type = unsafe {
            let ptr = $parser.next();
            ::std::ptr::read(ptr as *const $type)
        };
    };

    (
        @
        $amx:ident,
        $parser:ident,

        $arg:ident : ref $type:ty,
        $( $tail_arg:ident : $( $tail_data:ident )+ ),*
    ) => {
        expand_args!(@$amx, $parser, $arg : ref $type);
        expand_args!(@$amx, $parser, $( $tail_arg : $( $tail_data )+ ),*);
    };

    (
        @
        $amx:ident,
        $parser:ident,

        $arg:ident : $type:ty,
        $( $tail_arg:ident : $( $tail_data:ident )+ ),*
    ) => {
        expand_args!(@$amx, $parser, $arg : $type);
        expand_args!(@$amx, $parser, $( $tail_arg : $( $tail_data )+ ),*);
    };

    (
        $amx:ident,
        $params:ident,

        $( $arg:ident : $($data:ident)+ ),*
    ) => {
        let mut parser = $crate::args::Parser::new($params);
        expand_args!(@$amx, parser, $( $arg : $( $data )+ ),*);
    };
}