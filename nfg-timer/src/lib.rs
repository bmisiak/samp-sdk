use samp::amx::Amx;
use samp::cell::{AmxCell, AmxString, Ref, UnsizedBuffer, Buffer};
use samp::error::{AmxResult,AmxError};
use samp::plugin::SampPlugin;
use samp::{initialize_plugin, native};

use std::ffi::{CString,CStr};
use std::time::{Instant,Duration};
use std::collections::VecDeque;

use log::{info, error, debug};

/* These are the types of arguments we support */
#[derive(Debug, Clone, Copy)]
enum PassedArgument {
    Int(i32),
    Float(f32),
    Str(CString)
}

/* Internal struct representing a single scheduled timer */
#[derive(Debug, Clone, Copy)]
struct Timer {
    next_trigger: Instant,
    interval: Option<Duration>,
    passed_arguments: Vec<PassedArgument>,
    amx_identifier: samp::amx::AmxIdent,
    amx_callback_index: samp::amx::AmcExecIdx
}

impl Timer {
    pub fn trigger(&mut self) -> Result<(),i32> {
        /* Get the AMX which scheduled the timer */
        if let Some(amx) = samp::amx::get(self.amx_identifier) {
            /* Push the timer's arguments onto the AMX stack */
            for param in self.passed_arguments {
                let result = match param {
                    PassedArgument::Int(int_value) => amx.push(int_value),
                    PassedArgument::Float(float_value) => amx.push(float_value),
                    PassedArgument::Str(cstring) => {
                        let bytes = cstring.as_bytes();
                        let buffer = amx.allocator().allot_buffer(bytes.len() + 1);
                        let amx_str = unsafe { AmxString::new(buffer, bytes.as_ref()) }
                        amx.push(amx_str)
                    }
                }
            }

            if let Err(err) = amx.exec(self.amx_callback_index) {
                info!("Error executing callback");
            }
        } else {
            info!("Unable to find the amx related to a timer");
        }
    }
}

/* The plugin and its data: a list of scheduled timers */
struct PreciseTimers {
    timers: VecDeque<Timer>
}

impl PreciseTimers {
    #[native(raw)]
    pub fn create(&mut self, amx: &Amx, args: samp::args::Args) -> AmxResult<i32> {
        
        /* Get the basic timer parameters */
        let callback_name = args.next::<AmxString>().ok_or(AmxError::Params)?;
        let interval = args.next::<u32>().ok_or_else(|| AmxError::Params)?;
        let repeat = args.next::<bool>().ok_or_else(|| AmxError::Params)?;
        let argument_type_lettters = args.next::<AmxString>().ok_or_else(|| AmxError::Params)?.to_bytes(); //iterator on AmxString would be better if it was implemented

        /* Make sure they're sane */
        if(argument_type_lettters.len() != 4 + args.count()) {
            return AmxResult::Ok(0);
        }

        if(interval < 0) {
            return AmxResult::Ok(0);
        }

        /* Get the arguments to pass to the callback */
        let mut passed_arguments = Vec<PassedArgument>::with_capacity(argument_type_lettters.len());

        while let Some(arg) = args.next::<Ref>() {
            match argument_type_lettters.next() { //if Args implemented Iterator we could .zip args with letters
                Some(b'd') | Some(b'i') => {
                    passed_arguments.push(arg.as_cell());
                }
                Some(b's') => {
                    let buffer = UnsizedBuffer {
                        inner: arg
                    };
                    samp::cell::AmxString::from_raw()
                }
            }
        }

        /* Find the callback by name and save its index */
        let callback_index = amx.find_public(callback_name.to_string())?;
        
        /* Add the timer to the list */
        let timer_slot = self.timers.push_back(Timer {
            next_trigger: Instant::now() + Duration::from_millis(u64::from(interval)),
            interval: if repeat { Some(interval) } else { None },
            passed_arguments: passed_arguments,
            amx_identifier: samp::amx::AmxIdent::from(amx.ptr),
            amx_callback_index: callback_index
        })

        /* Return the timer's slot in Vec<> incresed by 1, so that 0 means invalid timer */
        AmxResult::Ok(timer_slot as i32 + 1)
    }

    #[native(name = "DeletePreciseTimer")]
    pub fn delete(&mut self, _: &Amx, timer_number: usize) -> AmxResult<i32> {
        /* Subtract 1 from the passed timer_number to get the actual Vec<> slot and remove it */
        match self.timers.swap_remove_back((timer_number - 1)) {
            Some(_timer) => AmxResult::Ok(1),
            None => AmxResult::Ok(0)
        }
    }
}

impl SampPlugin for PreciseTimers {
    fn on_load(&mut self) {
        info!("Precise timers loaded");
    }

    #[inline(always)]
    fn process_tick(&mut self) {
        // Rust's Instant is monotonic and nondecreasing. 
        // Works even during NTP time adjustment. 💖
        let now = Instant::now();

        self.timers.retain( |timer| {
            if(timer.next_trigger >= now) {
                // This executes the callback
                timer.trigger();
                
                if let Some(interval) = timer.interval {
                    timer.next_trigger = now + timer.interval;
                    //Retain timer, because it repeats
                    return true;
                } else {
                    //REMOVE timer, because it got triggered and does not repeat
                    return false;
                }
            } else {
                //Retain timer because it has yet to be triggered
                return true;
            }
        });
    }
}

initialize_plugin!(
    natives: [
        PreciseTimers::delete,
        PreciseTimers::create,
    ],
    {
        samp::plugin::enable_process_tick();

        // get a default samp logger (uses samp logprintf).
        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info); // logging only info, warn and error messages

        let log_file = fern::log_file("myplugin.log").expect("Something wrong!");

        // log trace and debug messages in an another file
        let trace_level = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                // all messages will be formated like
                // memcached error: something (error!("something"))
                // memcached info: some info (info!("some info"))
                callback.finish(format_args!("memcached {}: {}", record.level().to_string().to_lowercase(), message))
            })
            .chain(samp_logger)
            .chain(trace_level)
            .apply();
        
        return PreciseTimer {
            timers: VecDequeue::with_capacity(1000),
        };
    }
);
