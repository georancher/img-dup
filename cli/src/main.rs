#![feature(unsafe_destructor)]

extern crate getopts;
extern crate image;
extern crate img_hash;
extern crate libc;
extern crate serialize;
extern crate time;

use config::{parse_args, ProgramSettings};
use output::{output_results, test_outfile};
use processing::process;

use std::io::util::NullWriter;

use std::os;

macro_rules! json_insert(
    ($map:expr, $key:expr, $val:expr) => (
        $map.insert(::std::borrow::ToOwned::to_owned($key), $val.to_json())
    );
);

mod config;
mod output;

fn main() {
    let args = os::args();

    let settings = parse_args(args.as_slice());

	if settings.gui {
        show_gui(settings);
		return;
	}

    // Silence standard messages if we're outputting JSON
    let mut out = get_output(&settings);    

    match settings.outfile {
        Some(ref outfile) => {
            (writeln!(out, "Testing output file ({})...",
                outfile.display())).unwrap();
            test_outfile(outfile).unwrap();
        },
        None => (),        
    };
    
    out.write_line("Searching for images...").unwrap();

    let mut image_paths = processing::find_images(&settings);

    let image_count = image_paths.len();

    (writeln!(out, "Images found: {}", image_count)).unwrap();

    if settings.limit > 0 {
        (writeln!(out, "Limiting to: {}", settings.limit)).unwrap();
        image_paths.truncate(settings.limit);
    }

    (writeln!(out, "Processing images in {} threads. Please wait...\n", 
             settings.threads)).unwrap();

    let results = processing::process(&settings, image_paths);

    out.write_line("").unwrap();

    output::output_results(&settings, &results).unwrap()   
}

fn get_output(settings: &ProgramSettings) -> Box<Writer> {
    if settings.silent_stdout() {
        box NullWriter as Box<Writer> 
    } else {
        box std::io::stdio::stdout() as Box<Writer>
    }    
}
