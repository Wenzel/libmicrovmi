//! VirtualBox context, configured from the environment:
//! - `VBOX_APP_HOME`: VirtualBox bin directory (`VBoxManage`, `VBoxHeadless`, and the C API the
//!   driver loads); unset: `VBoxManage`/`VBoxHeadless` from `PATH`
//! - `VBOX_TEST_VM` (default `microvmi-alpine`), `VBOX_TEST_SNAPSHOT` (default `base`): a powered
//!   off or saved VM and an online snapshot of it, restored before each test
//! - `VBOX_TEST_MEMORY_MB`: if set, the snapshot's saved state is discarded and the guest cold boots
//!   with that much RAM; teardown restores the snapshot, so the VM's configuration is unchanged
//! - `VBOX_TEST_CPUS`: same, with that many vCPUs
//! - `VBOX_TEST_CORE_DIR` (default: the temp dir): where tests write core dumps, as large as the
//!   guest's RAM (avoid a RAM-backed tmpfs for big guests); deleted at teardown
//! - `VBOX_TEST_METHOD`: the access method, `machinedebugger` or `vboxvmi`, or the driver's default
//!   access method with VBoxVMI enabled (`default`) or not (`default-fallback`). Unset selects an
//!   access method compiled into the test binary. For `vboxvmi` and `default`, setup enables
//!   VBoxVMI on the VM while preserving the prior extradata value. The socket honors
//!   `VBoxInternal/VMI/SocketPath`; otherwise it is `<VirtualBox user home>/vmi/<VM UUID>.sock`.
use parking_lot::Mutex;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use log::debug;
use microvmi::api::params::{CommonInitParams, DriverInitParams, VBoxInitParams};
use microvmi::api::{DriverType, Introspectable};
use microvmi::init;

use crate::common::config::TIMEOUT;
use crate::common::context::Context;

/// The VM lifecycle is the same for every access method; only `init_driver` uses `method`.
pub struct VirtualBox {
    /// `None`: the driver's default access method.
    pub method: Option<VBoxInitParams>,
    /// `VBoxInternal/VMI/Enabled` while the VM runs.
    pub vmi_enabled: bool,
    prior_vmi_enabled: Mutex<Option<Option<String>>>,
}

impl VirtualBox {
    /// The context for the access method in `VBOX_TEST_METHOD`.
    pub fn from_env() -> Self {
        match env::var("VBOX_TEST_METHOD").as_deref() {
            Err(_) => VirtualBox::new(compiled_method()),
            Ok("machinedebugger") => VirtualBox::new(VBoxInitParams::MachineDebugger),
            Ok("vboxvmi") => VirtualBox::new(vboxvmi()),
            Ok("default") => VirtualBox::with_default_method(true),
            Ok("default-fallback") => VirtualBox::with_default_method(false),
            Ok(other) => panic!("unknown VBOX_TEST_METHOD {:?}", other),
        }
    }

    /// The context for `method`; VBoxVMI is enabled on the VM for the VBoxVMI method.
    pub fn new(method: VBoxInitParams) -> Self {
        let vmi_enabled = matches!(method, VBoxInitParams::VBoxVMI { .. });
        VirtualBox {
            method: Some(method),
            vmi_enabled,
            prior_vmi_enabled: Mutex::new(None),
        }
    }

    /// The context for the driver's default access method, with VBoxVMI enabled on the VM or not.
    pub fn with_default_method(vmi_enabled: bool) -> Self {
        VirtualBox {
            method: None,
            vmi_enabled,
            prior_vmi_enabled: Mutex::new(None),
        }
    }
}

fn vboxvmi() -> VBoxInitParams {
    VBoxInitParams::VBoxVMI {
        socket_path: Some(vboxvmi_socket()),
    }
}

/// The access method when `VBOX_TEST_METHOD` is unset: one compiled into the test binary.
#[cfg(feature = "virtualbox-api")]
fn compiled_method() -> VBoxInitParams {
    VBoxInitParams::MachineDebugger
}

#[cfg(all(not(feature = "virtualbox-api"), feature = "virtualbox-vmi"))]
fn compiled_method() -> VBoxInitParams {
    vboxvmi()
}

/// The configured or default VBoxVMI socket path.
pub fn vboxvmi_socket() -> String {
    if let Some(path) = extradata(VMI_SOCKET) {
        return path;
    }
    let home = env::var_os("VBOX_USER_HOME")
        .map(PathBuf::from)
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                env::current_dir().unwrap().join(p)
            }
        })
        .unwrap_or_else(|| {
            let home = PathBuf::from(env::var_os("HOME").expect("HOME is not set"));
            let legacy = home.join(".VirtualBox");
            if fs::metadata(&legacy).is_ok_and(|m| m.is_dir()) {
                legacy
            } else {
                env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .map(|p| if p.is_absolute() { p } else { home.join(p) })
                    .unwrap_or_else(|| home.join(".config"))
                    .join("VirtualBox")
            }
        });
    home.join("vmi")
        .join(format!("{}.sock", vm_info("UUID")))
        .display()
        .to_string()
}

const VMI_SOCKET: &str = "VBoxInternal/VMI/SocketPath";

const VMI_ENABLED: &str = "VBoxInternal/VMI/Enabled";

pub fn vm_name() -> String {
    env::var("VBOX_TEST_VM").unwrap_or_else(|_| String::from("microvmi-alpine"))
}

fn bin(name: &str) -> PathBuf {
    env::var_os("VBOX_APP_HOME")
        .map(|dir| PathBuf::from(dir).join(name))
        .unwrap_or_else(|| PathBuf::from(name))
}

pub fn try_vboxmanage(args: &[&str]) -> Result<String, String> {
    debug!("VBoxManage {}", args.join(" "));
    let out = Command::new(bin("VBoxManage"))
        .args(args)
        .output()
        .map_err(|e| format!("Failed to start VBoxManage: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// Returns an extradata value, or `None` when the key is unset.
pub fn extradata(key: &str) -> Option<String> {
    let output = vboxmanage(&["getextradata", &vm_name(), key]);
    output
        .strip_prefix("Value: ")
        .map(|value| value.trim_end().to_string())
}

/// Runs `VBoxManage args` and returns its stdout, panics on failure.
pub fn vboxmanage(args: &[&str]) -> String {
    try_vboxmanage(args).unwrap_or_else(|e| panic!("VBoxManage {} failed: {}", args.join(" "), e))
}

/// `VBoxManage showvminfo --machinereadable` value of `key`, unquoted.
pub fn vm_info(key: &str) -> String {
    let info = vboxmanage(&["showvminfo", &vm_name(), "--machinereadable"]);
    let prefix = format!("{}=", key);
    info.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no {} in showvminfo", key))
        .trim_matches('"')
        .to_string()
}

/// Core dump path, known to teardown so it is deleted even if a test times out.
pub fn core_path() -> PathBuf {
    let dir = env::var_os("VBOX_TEST_CORE_DIR").map_or_else(env::temp_dir, PathBuf::from);
    dir.join(format!("microvmi-test-{}.core", vm_name()))
}

/// Retries `f` until it succeeds, panics with its last error after 60s.
fn wait_for(what: &str, mut f: impl FnMut() -> Result<(), String>) {
    let start = Instant::now();
    while let Err(e) = f() {
        if start.elapsed() > Duration::from_secs(60) {
            panic!("timeout: {}: {}", what, e);
        }
        sleep(Duration::from_millis(500));
    }
}

/// Powers the VM off, then restores the snapshot.
fn power_off_and_restore() {
    let vm = vm_name();
    let snapshot = env::var("VBOX_TEST_SNAPSHOT").unwrap_or_else(|_| String::from("base"));
    // Retried: transient states (starting, saving, ...) must settle first, and poweroff returns
    // before VBoxHeadless exits and unlocks the machine, which restore needs.
    wait_for("power off and restore snapshot", || {
        let state = vm_info("VMState");
        if matches!(state.as_str(), "running" | "paused" | "stuck") {
            try_vboxmanage(&["controlvm", &vm, "poweroff"])?;
            return Err(format!("VM was {}", state));
        }
        try_vboxmanage(&["snapshot", &vm, "restore", &snapshot]).map(drop)
    });
}

impl VirtualBox {
    /// Driver params for the test VM and `method`.
    pub fn init_params(&self) -> DriverInitParams {
        DriverInitParams {
            common: Some(CommonInitParams { vm_name: vm_name() }),
            virtualbox: self.method.clone(),
            ..Default::default()
        }
    }
}

impl Context for VirtualBox {
    /// restore the snapshot and start the VM headless
    fn setup(&self) {
        debug!("setup test");
        power_off_and_restore();
        let vm = vm_name();
        let mut modify = Vec::new();
        for (var, option) in [
            ("VBOX_TEST_MEMORY_MB", "--memory"),
            ("VBOX_TEST_CPUS", "--cpus"),
        ] {
            if let Ok(value) = env::var(var) {
                modify.extend([option.to_string(), value]);
            }
        }
        if !modify.is_empty() {
            // a saved state can't change hardware: cold boot instead
            vboxmanage(&["discardstate", &vm]);
            let mut args = vec!["modifyvm", &vm];
            args.extend(modify.iter().map(String::as_str));
            vboxmanage(&args);
        }
        if self.vmi_enabled {
            let mut prior = self.prior_vmi_enabled.lock();
            assert!(
                prior.is_none(),
                "VBoxVMI setup called twice without teardown"
            );
            *prior = Some(extradata(VMI_ENABLED));
            vboxmanage(&["setextradata", &vm, VMI_ENABLED, "1"]);
        }
        // VBoxHeadless directly, not `startvm`, so it is the binary from VBOX_APP_HOME.
        let mut headless = Command::new(bin("VBoxHeadless"))
            .args(["--startvm", &vm])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start VBoxHeadless");
        // reaped once teardown powers the VM off
        std::thread::spawn(move || headless.wait());
        // list runningvms takes no machine lock, so it can't race VBoxHeadless' own.
        let running = format!("\"{}\" ", vm);
        wait_for("VM running", || {
            let vms = vboxmanage(&["list", "runningvms"]);
            match vms.lines().any(|l| l.starts_with(&running)) {
                true => Ok(()),
                false => Err(format!("not in list runningvms: {:?}", vms)),
            }
        });
    }

    fn init_driver(&self) -> Box<dyn Introspectable> {
        init(Some(DriverType::VirtualBox), Some(self.init_params()))
            .expect("Failed to init libmicrovmi")
    }

    /// power off and restore the snapshot, undoing any `VBOX_TEST_MEMORY_MB`/`VBOX_TEST_CPUS` change
    fn teardown(&self) {
        debug!("teardown test");
        power_off_and_restore();
        if self.vmi_enabled {
            let prior = self
                .prior_vmi_enabled
                .lock()
                .take()
                .expect("VBoxVMI teardown called without setup");
            match prior {
                Some(value) => vboxmanage(&["setextradata", &vm_name(), VMI_ENABLED, &value]),
                None => vboxmanage(&["setextradata", &vm_name(), VMI_ENABLED]),
            };
        }
        let _ = std::fs::remove_file(core_path());
    }

    /// Scaled with RAM size: a test may dump and read all of it.
    fn timeout(&self) -> Duration {
        let mib: u64 = vm_info("memory").parse().unwrap();
        Duration::from_secs(TIMEOUT + 10 * mib / 1024)
    }

    fn vcpu_count(&self) -> u16 {
        vm_info("cpus").parse().unwrap()
    }
}
