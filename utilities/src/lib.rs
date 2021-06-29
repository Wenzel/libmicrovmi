/// This crate implements utilities and common code shared by libmicrovmi examples
use clap::{Arg, ArgMatches};
use microvmi::api::params::{
    CommonInitParams, DriverInitParams, KVMInitParams, MemflowConnectorParams, MemflowInitParams,
};

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
            // memflow
            Arg::with_name("memflow_connector_name")
                .long("memflow_connector_name")
                .takes_value(true)
                .help("Driver parameter (optional for Memflow): Memflow connector name"),
            Arg::with_name("memflow_connector_args")
                .long("memflow_connector_args")
                .multiple(true)
                .min_values(1),
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
        let memflow = matches
            .value_of("memflow_connector_name")
            .map(|name| MemflowInitParams {
                connector_name: name.to_string(),
                connector_args: matches.values_of("memflow_connector_args").map(|v| {
                    MemflowConnectorParams::Default {
                        args: v.map(|s| s.to_string()).collect(),
                    }
                }),
            });
        DriverInitParams {
            common,
            kvm,
            memflow,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Clappable;
    use clap::App;
    use microvmi::api::params::{DriverInitParams, KVMInitParams, MemflowConnectorParams};

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

    // tests for memflow
    #[test]
    fn test_memflow_connector_name() {
        let cmdline = vec!["test", "--memflow_connector_name=foobar"];
        let matches = App::new("test")
            .args(DriverInitParams::to_clap_args().as_ref())
            .get_matches_from(cmdline);
        let params = DriverInitParams::from_matches(&matches);
        assert_eq!("foobar", params.memflow.unwrap().connector_name)
    }

    #[test]
    fn test_memflow_connector_args_one() {
        let cmdline = vec![
            "test",
            "--memflow_connector_name=foobar",
            "--memflow_connector_args",
            "first",
        ];
        let matches = App::new("test")
            .args(DriverInitParams::to_clap_args().as_ref())
            .get_matches_from(cmdline);
        let params = DriverInitParams::from_matches(&matches);
        assert_eq!(
            MemflowConnectorParams::Default {
                args: vec!["first".into()]
            },
            params.memflow.unwrap().connector_args.unwrap()
        )
    }

    #[test]
    fn test_memflow_connector_args_multiple() {
        let cmdline = vec![
            "test",
            "--memflow_connector_name=foobar",
            "--memflow_connector_args",
            "first",
            "second",
            "third",
        ];
        let matches = App::new("test")
            .args(DriverInitParams::to_clap_args().as_ref())
            .get_matches_from(cmdline);
        let params = DriverInitParams::from_matches(&matches);
        assert_eq!(
            MemflowConnectorParams::Default {
                args: vec!["first".into(), "second".into(), "third".into()]
            },
            params.memflow.unwrap().connector_args.unwrap()
        )
    }
}
