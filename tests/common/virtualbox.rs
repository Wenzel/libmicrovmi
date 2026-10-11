//! VirtualBox context, configured from the environment:
//! - `VBOX_APP_HOME`: VirtualBox bin directory (`VBoxManage`, `VBoxHeadless`, and the C API the
//!   driver loads); unset: `VBoxManage`/`VBoxHeadless` from `PATH`
//! - `VBOX_TEST_VM` (default `microvmi-alpine`), `VBOX_TEST_SNAPSHOT` (default `base`): a powered
//!   off or saved VM and an online snapshot of it, restored before each test
//! - `VBOX_TEST_MEMORY_MB`: if set, the snapshot's saved state is discarded and the guest cold boots
//!   with that much RAM; teardown restores the snapshot, so the VM's configuration is unchanged
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use log::debug;
use microvmi::api::params::{CommonInitParams, DriverInitParams, VBoxInitParams};
use microvmi::api::{DriverType, Introspectable};
use microvmi::init;

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

fn try_vboxmanage(args: &[&str]) -> Result<String, String> {
    debug!("VBoxManage {}", args.join(" "));
    let out = Command::new(bin("VBoxManage"))
        .args(args)
        .output()
        .expect("Failed to start VBoxManage");
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

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timeout: {}",
            what
        );
        sleep(Duration::from_millis(500));
    }
}

/// Powers the VM off if it is running, then restores the snapshot.
fn power_off_and_restore() {
    let vm = vm_name();
    if matches!(vm_info("VMState").as_str(), "running" | "paused") {
        vboxmanage(&["controlvm", &vm, "poweroff"]);
    }
    // poweroff returns before VBoxHeadless exits and unlocks the machine, which restore needs.
    let snapshot = env::var("VBOX_TEST_SNAPSHOT").unwrap_or_else(|_| String::from("base"));
    wait_for("snapshot restore", || {
        try_vboxmanage(&["snapshot", &vm, "restore", &snapshot]).is_ok()
    });
}

impl Context for VirtualBox {
    /// restore the snapshot and start the VM headless
    fn setup(&self) {
        debug!("setup test");
        power_off_and_restore();
        let vm = vm_name();
        if let Ok(mb) = env::var("VBOX_TEST_MEMORY_MB") {
            vboxmanage(&["discardstate", &vm]);
            vboxmanage(&["modifyvm", &vm, "--memory", &mb]);
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
            vboxmanage(&["list", "runningvms"])
                .lines()
                .any(|l| l.starts_with(&running))
        });
    }

    fn init_driver(&self) -> Box<dyn Introspectable> {
        init(
            Some(DriverType::VirtualBox),
            Some(DriverInitParams {
                common: Some(CommonInitParams { vm_name: vm_name() }),
                virtualbox: Some(self.method.clone()),
                ..Default::default()
            }),
        )
        .expect("Failed to init libmicrovmi")
    }

    /// power off and restore the snapshot, undoing any `VBOX_TEST_MEMORY_MB` change
    fn teardown(&self) {
        debug!("teardown test");
        power_off_and_restore();
    }
}
