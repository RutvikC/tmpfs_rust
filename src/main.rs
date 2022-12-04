// extern crates
extern crate fuse;
extern crate libc;
extern crate time;
#[macro_use]
extern crate log;
extern crate env_logger;

// Bindings
use tmpfs::TmpFS;
use std::env;
//use std::str;
//use std::ffi::OsStr;

fn main() {
    // Init log level system (error, warn, info, debug, trace) for this program
    env_logger::init();

    /* Extract the mountpoint from the command line argument
     * If the argument is not found generate an error and return
    */
    let args: Vec<String> = env::args().collect();

    let mountpoint = match env::args().nth(1) {
        Some(path) => path,
        None => {
            error!("Usage: {} <mount_point>. Provide mountpoint argument", env::args().nth(0).unwrap());
            return;
        }
    };
    let mut fs_max_size:u64 = 0;

    if env::args().len() != 3 {
        error!("Usage: <file_system_size>. Provide FS max size argument also");
        return;
    }
    else {
        let size = (&args[2]).parse::<u64>();
        match size {
            Ok(s) => fs_max_size = s,
            Err(_e) => {
                error!("The size entered is not an integer!");
                return;
            },
        }
    }
    // Create a file system instance
    let fs = TmpFS::new(fs_max_size);

    // Mount the file system using fuse's mount api
    fuse::mount(fs, &mountpoint, &[]).unwrap();
}