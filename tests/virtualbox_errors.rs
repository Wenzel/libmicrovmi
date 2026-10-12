//! VirtualBox driver misconfigurations: `init` fails with an error naming the cause.
//! The VBoxVMI cases need no VM: a missing socket is what a VM without VBoxVMI enabled leaves, and a
//! fake server stands in for a VirtualBox speaking another protocol.
#![cfg(any(feature = "virtualbox-api", feature = "virtualbox-vmi"))]

// kvm: tests/common then holds the KVM context, as in the integration tests
#[cfg(all(feature = "virtualbox-api", not(feature = "kvm")))]
#[allow(dead_code)]
mod common;

/// `init(driver_type, params)`'s error message; panics if init succeeds.
#[cfg(any(
    feature = "virtualbox-vmi",
    all(feature = "virtualbox-api", not(feature = "kvm"))
))]
fn init_error(
    driver_type: Option<microvmi::api::DriverType>,
    params: microvmi::api::params::DriverInitParams,
) -> String {
    match microvmi::init(driver_type, Some(params)) {
        Ok(_) => panic!("init succeeded"),
        Err(e) => {
            println!("init error: {}", e);
            e.to_string()
        }
    }
}

#[cfg(any(
    feature = "virtualbox-vmi",
    all(feature = "virtualbox-api", not(feature = "kvm"))
))]
#[test]
fn virtualbox_access_method_is_required() {
    use microvmi::api::params::DriverInitParams;
    use microvmi::api::DriverType;

    assert_eq!(
        init_error(Some(DriverType::VirtualBox), DriverInitParams::default()),
        "VirtualBox requires an explicit access method"
    );
}

#[cfg(feature = "virtualbox-vmi")]
mod vbox_vmi {
    use std::fs::DirBuilder;
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use microvmi::api::params::{DriverInitParams, VBoxInitParams};
    use microvmi::api::DriverType;
    use nix::sys::socket::{
        accept, bind, listen, send, setsockopt, socket, sockopt, AddressFamily, MsgFlags, SockAddr,
        SockFlag, SockType, UnixAddr,
    };
    use nix::sys::time::{TimeVal, TimeValLike};

    use super::init_error;

    /// Private temp dir, removed on drop. Its path is short, to fit a socket path in `sun_path`.
    struct TempDir(PathBuf);

    impl TempDir {
        /// Fails, rather than reuse it, if the path exists.
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let name = format!(
                "microvmi-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            DirBuilder::new().mode(0o700).create(&path).unwrap();
            TempDir(path)
        }

        fn socket(&self) -> PathBuf {
            self.0.join("vmi.sock")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn vboxvmi(dir: &TempDir) -> DriverInitParams {
        DriverInitParams {
            virtualbox: Some(VBoxInitParams::VBoxVMI {
                socket_path: dir.socket().to_str().unwrap().to_string(),
            }),
            ..Default::default()
        }
    }

    /// Serves one connection with a VBoxVMI greeting (`VMIMSGHDR` + `VMIHELLO`) of `magic`,
    /// `version`, then returns `init`'s error.
    fn init_error_with_greeting(magic: u32, version: u32) -> String {
        let dir = TempDir::new();
        let raw = socket(
            AddressFamily::Unix,
            SockType::SeqPacket,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .unwrap();
        let listener = unsafe { OwnedFd::from_raw_fd(raw) };
        let fd = listener.as_raw_fd();
        bind(fd, &SockAddr::Unix(UnixAddr::new(&dir.socket()).unwrap())).unwrap();
        listen(fd, 1).unwrap();
        // accept fails instead of blocking forever if the client never connects
        setsockopt(fd, sockopt::ReceiveTimeout, &TimeVal::seconds(10)).unwrap();
        let server = thread::spawn(move || {
            let conn = unsafe { OwnedFd::from_raw_fd(accept(listener.as_raw_fd()).unwrap()) };
            let mut msg = Vec::new();
            // size, id, kind (hello), flags, rc
            msg.extend_from_slice(&24u32.to_le_bytes());
            msg.extend_from_slice(&0u32.to_le_bytes());
            msg.extend_from_slice(&1u16.to_le_bytes());
            msg.extend_from_slice(&0u16.to_le_bytes());
            msg.extend_from_slice(&0i32.to_le_bytes());
            msg.extend_from_slice(&magic.to_le_bytes());
            msg.extend_from_slice(&version.to_le_bytes());
            send(conn.as_raw_fd(), &msg, MsgFlags::empty()).unwrap();
        });
        let err = init_error(Some(DriverType::VirtualBox), vboxvmi(&dir));
        server.join().expect("fake VBoxVMI server failed");
        err
    }

    fn assert_socket_missing(err: &str) {
        assert!(
            err.contains("cannot connect to the VBoxVMI socket"),
            "{}",
            err
        );
        assert!(err.contains("VBoxInternal/VMI/Enabled"), "{}", err);
    }

    #[test]
    fn vboxvmi_socket_missing() {
        let dir = TempDir::new();
        assert_socket_missing(&init_error(Some(DriverType::VirtualBox), vboxvmi(&dir)));
    }

    /// Autodetection reports the VirtualBox error too, since the params target VirtualBox.
    #[test]
    fn vboxvmi_socket_missing_autodetect() {
        let dir = TempDir::new();
        assert_socket_missing(&init_error(None, vboxvmi(&dir)));
    }

    #[test]
    fn vboxvmi_version_mismatch() {
        let err = init_error_with_greeting(vboxvmi::MAGIC, vboxvmi::VERSION + 1);
        assert!(err.contains("VBoxVMI protocol version mismatch"), "{}", err);
    }

    #[test]
    fn vboxvmi_bad_magic() {
        let err = init_error_with_greeting(0xdead_beef, vboxvmi::VERSION);
        assert!(err.contains("not a VBoxVMI server"), "{}", err);
    }
}

/// MachineDebugger against a stock VirtualBox: run with `--ignored` and `VBOX_APP_HOME` set to its
/// bin directory; the VM is configured as for the integration tests.
#[cfg(all(feature = "virtualbox-api", not(feature = "kvm")))]
#[test]
#[ignore = "needs a test VM and VBOX_APP_HOME set to a stock VirtualBox"]
fn machinedebugger_unpatched_virtualbox() {
    use common::context::Context;
    use common::virtualbox::VirtualBox;
    use microvmi::api::params::VBoxInitParams;
    use microvmi::api::DriverType;

    assert!(
        std::env::var_os("VBOX_APP_HOME").is_some(),
        "set VBOX_APP_HOME to a stock VirtualBox bin directory"
    );
    struct Teardown(VirtualBox);
    impl Drop for Teardown {
        fn drop(&mut self) {
            self.0.teardown();
        }
    }
    let ctx = Teardown(VirtualBox::new(VBoxInitParams::MachineDebugger));
    ctx.0.setup();
    let err = init_error(Some(DriverType::VirtualBox), ctx.0.init_params());
    assert!(
        err.contains("does not implement IMachineDebugger::readPhysicalMemory"),
        "{}",
        err
    );
}
