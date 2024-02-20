use crate::api::params::{DriverInitParams, MemflowConnectorParams};
use crate::api::{DriverType, Introspectable};
use std::error::Error;

use memflow::mem::PhysicalMemory;
use memflow::plugins::{Args, ConnectorArgs, ConnectorInstanceArcBox, Inventory};
use memflow::types::PhysicalAddress;
use std::cell::RefCell;

#[derive(thiserror::Error, Debug)]
pub enum MemflowDriverError {
    #[error("Memfow driver initialization requires a connector parameter")]
    MissingConnectorParameter,
    #[error("Invalid format for Memflow connector argument (key=value), got {0}")]
    InvalidConnectorArgument(String),
}

const QEMU_PROCFS_CONNECTOR_NAME: &str = "qemu_procfs";

pub struct Memflow {
    // refcell required because read methods are mutable
    // contrary to our read_frame signature
    connector: RefCell<ConnectorInstanceArcBox<'static>>,
}

impl Memflow {
    pub fn new(init_params: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        info!("init Memflow");
        // check connector name
        let memflow_init_params = init_params
            .memflow
            .ok_or(MemflowDriverError::MissingConnectorParameter)?;
        // parse connector args
        let mut extra_args = Args::new();
        // reuse some of the common parameters in init_params
        #[allow(clippy::single_match)]
        match memflow_init_params.connector_name.as_str() {
            QEMU_PROCFS_CONNECTOR_NAME => {
                // if init_params.common.vm_name exists and connector_name is qemu_procfs
                // then insert vm_name value as 'name' connector args
                if init_params.common.is_some() {
                    extra_args = extra_args.insert("name", &init_params.common.unwrap().vm_name);
                }
            }
            _ => {}
        };
        if memflow_init_params.connector_args.is_some() {
            let MemflowConnectorParams::Default { args } =
                memflow_init_params.connector_args.unwrap();

            // for each string, split at '=' to get key, value
            for s in args.iter() {
                let (key, value) = s
                    .split_once('=')
                    .ok_or_else(|| MemflowDriverError::InvalidConnectorArgument(s.clone()))?;
                // push it into memflow Args type
                extra_args = extra_args.insert(key, value);
            }
        }
        // display final connector args
        debug!("Memflow connector args: {:#?}", extra_args);
        let create_connector_args = ConnectorArgs::new(None, extra_args, None);
        // create inventory
        let inventory = Inventory::scan();
        // create memflow connector
        let connector = inventory.create_connector(
            &memflow_init_params.connector_name,
            None,
            Some(&create_connector_args),
        )?;
        Ok(Memflow {
            connector: RefCell::new(connector),
        })
    }
}

impl Introspectable for Memflow {
    fn read_physical(
        &self,
        paddr: u64,
        buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        self.connector
            .borrow_mut()
            .phys_read_into(PhysicalAddress::from(paddr), buf)?;
        *bytes_read = buf.len() as u64;
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.connector.borrow_mut().metadata().max_address.to_umem())
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::Memflow
    }
}
