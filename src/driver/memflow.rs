use crate::api::params::{DriverInitParams, MemflowConnectorParams};
use crate::api::{DriverType, Introspectable};
use std::error::Error;

use memflow::connector::{ConnectorArgs, ConnectorInstance, ConnectorInventory};
use memflow::{PhysicalAddress, PhysicalMemory};
use std::cell::RefCell;

#[derive(thiserror::Error, Debug)]
pub enum MemflowDriverError {
    #[error("Memfow driver initialization requires a connector parameter")]
    MissingConnectorParameter,
    #[error("Invalid format for Memflow connector argument (key=value), got {0}")]
    InvalidConnectorArgument(String),
}

pub struct Memflow {
    // refcell required because read methods are mutable
    // contrary to our read_frame signature
    connector: RefCell<ConnectorInstance>,
}

impl Memflow {
    pub fn new(init_params: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        info!("init Memflow");
        // check connector name
        let memflow_init_params = init_params
            .memflow
            .ok_or(MemflowDriverError::MissingConnectorParameter)?;
        // create inventory
        let inventory = unsafe { ConnectorInventory::scan() };
        // parse connector args
        let mut create_connector_args = ConnectorArgs::new();
        if memflow_init_params.connector_args.is_some() {
            let MemflowConnectorParams::Default { args } =
                memflow_init_params.connector_args.unwrap();
            // for each string, split at '=' to get key, value
            for s in args.iter() {
                let (key, value) = s
                    .split_once("=")
                    .ok_or_else(|| MemflowDriverError::InvalidConnectorArgument(s.clone()))?;
                // push it into memflow ConnectorArgs type
                create_connector_args = create_connector_args.insert(key, value);
            }
        }
        // create memflow connector
        debug!(
            "Memflow: create connector - name: {}, args: {:#?}",
            &memflow_init_params.connector_name, &create_connector_args
        );
        let connector = unsafe {
            inventory
                .create_connector(&memflow_init_params.connector_name, &create_connector_args)?
        };
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
        Ok(self.connector.borrow_mut().metadata().size as u64)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::Memflow
    }
}
