//! libmicrovmi is a cross-platform unified virtual machine introsection interface, following a simple design to keep interoperability at heart.
//!
//! Click on this [book 📖](https://wenzel.github.io/libmicrovmi/) to find our project documentation.
//!
//! The library's entrypoint is the [init](fn.init.html) function.

#![allow(clippy::upper_case_acronyms)]

pub mod api;
pub mod capi;
mod driver;
pub mod errors;

#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;

use enum_iterator::IntoEnumIterator;

use api::params::DriverInitParams;
use api::DriverType;
use api::Introspectable;
#[cfg(feature = "kvm")]
use driver::kvm::Kvm;
#[cfg(feature = "mflow")]
use driver::memflow::Memflow;
#[cfg(feature = "virtualbox-api")]
use driver::virtualbox_api::VBoxApi;
#[cfg(feature = "virtualbox-vmi")]
use driver::virtualbox_vmi::VBoxVmi;
#[cfg(feature = "xen")]
use driver::xen::Xen;
use errors::MicrovmiError;
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

/// libmicrovmi initialization entrypoint
///
/// This function will initialize a libmicrovmi driver and call the hypervisor VMI API.
/// It returns a `Box<dyn Introspectable>` trait object, which implements the [Introspectable](api/trait.Introspectable.html) trait.
///
/// For complete documentation on driver init params, please check [DriverInitParams](struct.DriverInitParams.html) struct.
///
/// # Arguments
/// * `driver_type`: optional driver type to initialize. If None, all compiled drivers will be initialized one by one. The first that succeeds will be returned; if `init_params.virtualbox` is set, a VirtualBox driver error is returned rather than trying the next driver.
/// * `init_params`: optional driver initialization parameters
///
/// # Examples
/// ```no_run
/// use microvmi::init;
/// // 1 - attempt to init all drivers, without any init parameters
/// let drv = init(None, None);
///
/// // 2 - add parameters: vm_name
/// // a `vm_name` parameter is required for multiple drivers: Xen, KVM, VirtualBox
/// use microvmi::api::params::{DriverInitParams, CommonInitParams};
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     ..Default::default()
/// };
/// let drv = init(None, Some(init_params));
///
/// // 3 - add parameters: KVM specific params
/// // KVM requires an additional unix socket to be specified
/// // and specify the KVM driver only
/// use microvmi::api::params::KVMInitParams;
/// use microvmi::api::DriverType;
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     kvm: Some(KVMInitParams::UnixSocket {path: String::from("/tmp/introspector")}),
///     ..Default::default()
/// };
/// let drv = init(Some(DriverType::KVM), Some(init_params));
/// ```
pub fn init(
    driver_type: Option<DriverType>,
    init_params: Option<DriverInitParams>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    info!("Microvmi init");
    debug!("Microvmi init params: {:#?}", init_params);
    match driver_type {
        None => {
            // for each possible DriverType
            for drv_type in DriverType::into_enum_iter() {
                // try to init
                match init_driver(drv_type, init_params.clone()) {
                    Ok(driver) => {
                        info!("Driver initialized: {:?}", driver.get_driver_type());
                        return Ok(driver);
                    }
                    // VirtualBox params select an access method: the caller targets VirtualBox.
                    Err(e)
                        if drv_type == DriverType::VirtualBox
                            && init_params.as_ref().is_some_and(|p| p.virtualbox.is_some()) =>
                    {
                        return Err(e)
                    }
                    Err(e) => {
                        debug!("{:?} driver initialization failed: {}", drv_type, e);
                        continue;
                    }
                }
            }
            info!("No driver available");
            Err(MicrovmiError::NoDriverAvailable)
        }
        Some(drv_type) => match init_driver(drv_type, init_params) {
            Ok(driver) => {
                info!("Driver initialized: {:?}", driver.get_driver_type());
                Ok(driver)
            }
            Err(e) => {
                debug!("{:?} driver initialization failed: {}", driver_type, e);
                Err(e)
            }
        },
    }
}

/// Initialize a given driver type
/// return None if the requested driver has not been compiled in libmicrovmi
fn init_driver(
    driver_type: DriverType,
    init_params_option: Option<DriverInitParams>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    let _init_params = init_params_option.unwrap_or(DriverInitParams {
        ..Default::default()
    });
    #[allow(clippy::match_single_binding)]
    match driver_type {
        #[cfg(feature = "kvm")]
        DriverType::KVM => Ok(Box::new(Kvm::new(create_kvmi()?, _init_params)?)),
        #[cfg(feature = "mflow")]
        DriverType::Memflow => Ok(Box::new(Memflow::new(_init_params)?)),
        #[cfg(any(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
        DriverType::VirtualBox => init_virtualbox(_init_params),
        #[cfg(feature = "xen")]
        DriverType::Xen => Ok(Box::new(Xen::new(_init_params)?)),
        #[allow(unreachable_patterns)]
        _ => Err(MicrovmiError::DriverNotCompiled(driver_type)),
    }
}

/// Initialize the VirtualBox driver with the access method selected by `init_params.virtualbox`,
/// or the default one when it is `None`
#[cfg(any(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
fn init_virtualbox(
    init_params: DriverInitParams,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    use api::params::VBoxInitParams;
    match &init_params.virtualbox {
        #[cfg(feature = "virtualbox-api")]
        Some(VBoxInitParams::MachineDebugger) => Ok(Box::new(VBoxApi::new(init_params)?)),
        #[cfg(not(feature = "virtualbox-api"))]
        Some(VBoxInitParams::MachineDebugger) => Err(MicrovmiError::AccessMethodNotCompiled {
            driver: DriverType::VirtualBox,
            method: "MachineDebugger",
            feature: "virtualbox-api",
        }),
        #[cfg(feature = "virtualbox-vmi")]
        Some(VBoxInitParams::VBoxVMI {
            socket_path: Some(socket_path),
        }) => Ok(Box::new(VBoxVmi::new(std::path::Path::new(socket_path))?)),
        #[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
        Some(VBoxInitParams::VBoxVMI { socket_path: None }) => Ok(Box::new(VBoxVmi::new(
            &driver::virtualbox_api::vmi_socket_path(&init_params)?,
        )?)),
        #[cfg(all(not(feature = "virtualbox-api"), feature = "virtualbox-vmi"))]
        Some(VBoxInitParams::VBoxVMI { socket_path: None }) | None => {
            Err(MicrovmiError::VBoxVmiSocketPathRequired)
        }
        #[cfg(not(feature = "virtualbox-vmi"))]
        Some(VBoxInitParams::VBoxVMI { .. }) => Err(MicrovmiError::AccessMethodNotCompiled {
            driver: DriverType::VirtualBox,
            method: "VBoxVMI",
            feature: "virtualbox-vmi",
        }),
        #[cfg(all(feature = "virtualbox-api", not(feature = "virtualbox-vmi")))]
        None => {
            info!("VirtualBox: MachineDebugger access method (the only one compiled)");
            Ok(Box::new(VBoxApi::new(init_params)?))
        }
        #[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
        None => {
            let socket = driver::virtualbox_api::vmi_socket_path(&init_params)?;
            match VBoxVmi::new(&socket) {
                Ok(driver) => {
                    info!("VirtualBox: VBoxVMI access method, socket {:?}", socket);
                    Ok(Box::new(driver))
                }
                Err(e) if driver::virtualbox_vmi::is_socket_missing(&*e) => {
                    info!(
                        "VirtualBox: MachineDebugger access method, falling back: no VBoxVMI socket {:?} (VBoxVMI not enabled on the VM)",
                        socket
                    );
                    Ok(Box::new(VBoxApi::new(init_params)?))
                }
                Err(e) => Err(e.into()),
            }
        }
    }
}
