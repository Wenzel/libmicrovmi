use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use clap::{App, Arg, ArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use log::trace;

use microvmi::api::DriverInitParam;
use microvmi::Microvmi;

const BUFFER_SIZE: usize = 64535; // 64K

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps VM physical memory")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .arg(
            Arg::with_name("kvmi_socket")
                .short("k")
                .takes_value(true)
                .help(
                "pass additional KVMi socket initialization parameter required for the KVM driver",
            ),
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
    let domain_name = matches.value_of("vm_name").unwrap();

    let dump_path = Path::new(
        matches
            .value_of("output")
            .map_or(&*format!("{}.dump", domain_name), |s| s),
    )
    .to_path_buf();
    let mut dump_file = File::create(&dump_path).expect("Fail to open dump file");
    dump_path.canonicalize().unwrap();

    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));

    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(200);
    spinner.set_message("Initializing libmicrovmi...");
    let mut drv =
        Microvmi::new(domain_name, None, init_option).expect("Failed to init libmicrovmi");
    spinner.finish_and_clear();

    println!("pausing the VM");
    drv.pause().expect("Failed to pause VM");

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

    for cur_addr in (0..max_addr).step_by(BUFFER_SIZE) {
        trace!(
            "reading {:#X} bytes of memory at {:#X}",
            BUFFER_SIZE,
            cur_addr
        );
        // reset buffer each loop
        let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
        let mut _bytes_read = 0;
        drv.read_exact(&mut buffer)
            .expect(&*format!("Failed to read memory at {:#X}", cur_addr));
        dump_file
            .write_all(&buffer)
            .expect("failed to write to file");
        // update bar
        bar.set_prefix(&*format!("{:#X}", cur_addr));
        bar.inc(BUFFER_SIZE as u64);
    }
    bar.finish();
    println!(
        "Finished dumping physical memory at {}",
        dump_path.display()
    );

    println!("resuming the VM");
    drv.resume().expect("Failed to resume VM");
}
