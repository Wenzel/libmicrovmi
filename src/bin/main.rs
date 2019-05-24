extern crate microvmi;

fn main() {
    println!("hello world !");

    microvmi::vmi_init();
    microvmi::vmi_close();
}
