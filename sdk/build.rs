use yansi::Color::{Blue, Red, Yellow};

fn main() {
    if cfg!(not(target_arch = "x86")) {
        eprintln!("{} {}.",
            Red.paint("Error:").bold(), 
            "samp-sdk compiles and works only on x86 architecture because the SA:MP server is built for this arch",
        );

        eprintln!(
            "{} {} {} {} {}.",
            Blue.paint("Hint:").bold(),
            "Try to install x86 version of compiler by typing",
            Yellow.paint("\"rustup install nightly-i686\""),
            "and then use it to build the plugin",
            Yellow.paint("\"cargo +nightly-i686 build\""),
        );

        panic!("Aborting compilation due to incompatible architecture.");
    }
}
