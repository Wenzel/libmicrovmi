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
//!   VBoxVMI and the debugger console (TCP port 5000, for the debugger exclusion test) on the VM
//!   while preserving the prior extradata values. The socket honors
//!   `VBoxInternal/VMI/SocketPath`; otherwise it is `<VirtualBox user home>/vmi/<VM UUID>.sock`.
//! - `VBOX_TEST_HIDDEN_PAGES` (default 16): `VBoxInternal/VMI/HiddenPages`, set for every method
//!   (it moves the RAM layout, so it must be the value the snapshot was saved with) and restored
//!   at teardown
//! - `VBOX_TEST_IEM`: if set, `VBoxInternal/EM/IemExecutesAll` runs all guest code in VirtualBox's
//!   instruction emulator (IEM) instead of VT-x, to test the VBoxVMI paths IEM takes; restored at
//!   teardown
use parking_lot::Mutex;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
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
    /// `VBoxInternal/VMI/Enabled` and `VBoxInternal/DBGC/Enabled` while the VM runs.
    pub vmi_enabled: bool,
    /// The extradata setup changed, with their values before setup; empty outside a test.
    prior_extradata: Mutex<Vec<(&'static str, Option<String>)>>,
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
            prior_extradata: Mutex::new(Vec::new()),
        }
    }

    /// The context for the driver's default access method, with VBoxVMI enabled on the VM or not.
    pub fn with_default_method(vmi_enabled: bool) -> Self {
        VirtualBox {
            method: None,
            vmi_enabled,
            prior_extradata: Mutex::new(Vec::new()),
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

/// The configured or default VBoxVMI socket path, fixed for the VM: looked up once, a VBoxManage
/// call takes about 1 s.
pub fn vboxvmi_socket() -> String {
    static SOCKET: LazyLock<String> = LazyLock::new(lookup_vboxvmi_socket);
    SOCKET.clone()
}

fn lookup_vboxvmi_socket() -> String {
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

/// Set to 1 while VBoxVMI is enabled.
const VMI_EXTRADATA: [&str; 2] = ["VBoxInternal/VMI/Enabled", "VBoxInternal/DBGC/Enabled"];
const VMI_HIDDEN_PAGES: &str = "VBoxInternal/VMI/HiddenPages";
const IEM_EXECUTES_ALL: &str = "VBoxInternal/EM/IemExecutesAll";

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
    for attempt in 0.. {
        let out = Command::new(bin("VBoxManage"))
            .args(args)
            .output()
            .map_err(|e| format!("Failed to start VBoxManage: {}", e))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        // VBoxSVC exits a few seconds after its last client: a client connecting meanwhile fails
        if attempt == 5 || !stderr.contains("Failed to create the VirtualBox object") {
            return Err(stderr);
        }
        sleep(Duration::from_secs(1));
    }
    unreachable!()
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
        if matches!(state.as_str(), "running" | "paused" | "stuck" | "gurumeditation") {
            try_vboxmanage(&["controlvm", &vm, "poweroff"])?;
            return Err(format!("VM was {}", state));
        }
        try_vboxmanage(&["snapshot", &vm, "restore", &snapshot]).map(drop)
    });
}

/// Types `ls /` and Enter at the guest console: a new process (`login` at the login prompt, `ls`
/// in a shell), whose address space switches write CR3. The idle test guest writes none.
pub fn start_guest_process() {
    type_line("ls /");
}

/// Types `line` and Enter at the guest console.
pub fn type_line(line: &str) {
    let vm = vm_name();
    vboxmanage(&["controlvm", &vm, "keyboardputstring", line]);
    vboxmanage(&["controlvm", &vm, "keyboardputscancode", "1c", "9c"]);
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
        let hidden_pages = env::var("VBOX_TEST_HIDDEN_PAGES").unwrap_or_else(|_| String::from("16"));
        let mut settings = vec![(VMI_HIDDEN_PAGES, Some(hidden_pages.as_str()))];
        settings.push((IEM_EXECUTES_ALL, env::var_os("VBOX_TEST_IEM").map(|_| "1")));
        // unset, not left as the VM has it: the default method then falls back, and no VBoxVMI
        // session runs beside the MachineDebugger method
        settings.extend(VMI_EXTRADATA.map(|key| (key, self.vmi_enabled.then_some("1"))));
        let mut prior = self.prior_extradata.lock();
        assert!(prior.is_empty(), "setup called twice without teardown");
        *prior = settings.iter().map(|&(key, _)| (key, extradata(key))).collect();
        for (key, value) in settings {
            let mut args = vec!["setextradata", &vm, key];
            args.extend(value);
            vboxmanage(&args);
        }
        drop(prior);
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
        for (key, value) in std::mem::take(&mut *self.prior_extradata.lock()) {
            match value {
                Some(value) => vboxmanage(&["setextradata", &vm_name(), key, &value]),
                None => vboxmanage(&["setextradata", &vm_name(), key]),
            };
        }
        let _ = std::fs::remove_file(core_path());
    }

    /// 40 s above the default for the VBoxManage calls (about 1 s each, up to 32 in the disconnect
    /// race test), scaled with RAM size: a test may dump and read all of it.
    fn timeout(&self) -> Duration {
        let mib: u64 = vm_info("memory").parse().unwrap();
        Duration::from_secs(TIMEOUT + 40 + 10 * mib / 1024)
    }

    fn start_guest_process(&self) {
        start_guest_process()
    }

    fn vcpu_count(&self) -> u16 {
        vm_info("cpus").parse().unwrap()
    }
}
