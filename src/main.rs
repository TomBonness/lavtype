#![allow(dead_code, unused_imports)]

use lavtype::app::Coordinator;

fn main() {
    if let Err(error) = Coordinator::run() {
        eprintln!("lavtype: {error}");
        std::process::exit(1);
    }
}
