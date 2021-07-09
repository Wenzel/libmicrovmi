/// This crate implements utilities and common code shared by libmicrovmi examples
use clap::{Arg, ArgMatches};
use microvmi::api::params::{CommonInitParams, DriverInitParams, KVMInitParams};

/// This trait allows to convert a struct to Clap's command line arguments
/// and to parse back the matches into the struct
pub trait Clappable {
    /// produces an equivalent of the struct as vector of Clap arguments
    fn to_clap_args<'a, 'b>() -> Vec<Arg<'a, 'b>>;
    /// builds a new struct from Clap matches
    fn from_matches(matches: &ArgMatches) -> Self;
}

impl Clappable for DriverInitParams {
    fn to_clap_args<'a, 'b>() -> Vec<Arg<'a, 'b>> {
        vec![
            // common
            Arg::with_name("vm_name")
                .long("vm_name")
                .takes_value(true)
                .help("Driver parameter (required for Xen, KVM, VirtualBox): VM name"),
            // kvm
            Arg::with_name("kvm_unix_socket")
                .long("kvm_unix_socket")
                .takes_value(true)
                .help("Driver parameter (required for KVM): KVM unix socket path"),
        ]
    }

    fn from_matches(matches: &ArgMatches) -> Self {
        let common = matches.value_of("vm_name").map(|s| CommonInitParams {
            vm_name: String::from(s),
        });
        let kvm = matches
            .value_of("kvm_unix_socket")
            .map(|s| KVMInitParams::UnixSocket {
                path: String::from(s),
            });
        DriverInitParams {
            common,
            kvm,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Clappable;
    use clap::App;
    use microvmi::api::params::{DriverInitParams, KVMInitParams};

    #[test]
    fn test_common_vm_name() {
        let cmdline = vec!["test", "--vm_name=windows10"];
        let matches = App::new("test")
            .args(DriverInitParams::to_clap_args().as_ref())
            .get_matches_from(cmdline);
        let params = DriverInitParams::from_matches(&matches);
        assert_eq!("windows10", params.common.unwrap().vm_name)
    }

    #[test]
    fn test_kvm_unix_socket() {
        let cmdline = vec!["test", "--kvm_unix_socket=/tmp/introspector"];
        let matches = App::new("test")
            .args(DriverInitParams::to_clap_args().as_ref())
            .get_matches_from(cmdline);
        let params = DriverInitParams::from_matches(&matches);
        assert_eq!(
            KVMInitParams::UnixSocket {
                path: String::from("/tmp/introspector")
            },
            params.kvm.unwrap()
        );
    }
}
