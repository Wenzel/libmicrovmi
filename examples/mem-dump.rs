use std::fs::File;
use std::io::Write;
use std::path::Path;

use clap::{App, Arg, ArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, trace};

use microvmi::api::params::DriverInitParams;
use microvmi::api::Introspectable;

use utilities::Clappable;

const PAGE_SIZE: usize = 4096;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps VM physical memory")
        .args(DriverInitParams::to_clap_args().as_ref())
        .arg(
            Arg::with_name("no-pause")
                .long("no-pause")
                .required(false)
                .takes_value(false)
                .help("Don't pause the VM while dumping the memory"),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .takes_value(true)
                .help("Output path"),
        )
        .get_matches()
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let init_params = DriverInitParams::from_matches(&matches);
    let domain_name: String = init_params
        .common
        .clone()
        .map_or(String::from("unknown_vm_name"), |v| v.vm_name);
    let no_pause = matches.is_present("no-pause");
    let dump_path = Path::new(
        matches
            .value_of("output")
            .map_or(&*format!("{}.dump", domain_name), |s| s),
    )
    .to_path_buf();
    let mut dump_file = File::create(&dump_path).expect("Fail to open dump file");
    dump_path.canonicalize().unwrap();

    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(200);
    spinner.set_message("Initializing libmicrovmi...");

    let mut drv: Box<dyn Introspectable> =
        microvmi::init(None, Some(init_params)).expect("Failed to init libmicrovmi");
    spinner.finish_and_clear();

    if !no_pause {
        println!("pausing the VM");
        drv.pause().expect("Failed to pause VM");
    }

    let max_addr = drv.get_max_physical_addr().unwrap();
    println!(
        "Dumping physical memory to {} until {:#X}",
        dump_path.file_name().unwrap().to_str().unwrap(),
        max_addr
    );

    let bar = ProgressBar::new(max_addr);
    bar.set_style(ProgressStyle::default_bar().template(
        "{prefix} {wide_bar} {bytes_per_sec} • {bytes}/{total_bytes} • {percent}% • {elapsed}",
    ));
    // redraw every 0.1% change, otherwise it becomes the bottleneck
    bar.set_draw_delta(max_addr / 1000);

    for cur_addr in (0..max_addr).step_by(PAGE_SIZE) {
        trace!(
            "reading {:#X} bytes of memory at {:#X}",
            PAGE_SIZE,
            cur_addr
        );
        // reset buffer each loop
        let mut buffer: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
        let mut _bytes_read = 0;
        drv.read_physical(cur_addr, &mut buffer, &mut _bytes_read)
            .unwrap_or_else(|_| debug!("failed to read memory at {:#X}", cur_addr));
        dump_file
            .write_all(&buffer)
            .expect("failed to write to file");
        // update bar
        bar.set_prefix(&*format!("{:#X}", cur_addr));
        bar.inc(PAGE_SIZE as u64);
    }
    bar.finish();
    println!(
        "Finished dumping physical memory at {}",
        dump_path.display()
    );

    if !no_pause {
        println!("resuming the VM");
        drv.resume().expect("Failed to resume VM");
    }
}
