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
use std::env;
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
    pub method: VBoxInitParams,
}

pub fn vm_name() -> String {
    env::var("VBOX_TEST_VM").unwrap_or_else(|_| String::from("microvmi-alpine"))
}

fn bin(name: &str) -> PathBuf {
    env::var_os("VBOX_APP_HOME")
        .map(|dir| PathBuf::from(dir).join(name))
        .unwrap_or_else(|| PathBuf::from(name))
}

pub(crate) fn try_vboxmanage(args: &[&str]) -> Result<String, String> {
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
            virtualbox: Some(self.method.clone()),
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
