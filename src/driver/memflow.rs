use crate::api::params::{DriverInitParams, MemflowConnectorParams};
use crate::api::{DriverType, Introspectable};
use std::error::Error;

use memflow::connector::{ConnectorArgs, ConnectorInventory};

#[derive(thiserror::Error, Debug)]
pub enum MemflowDriverError {
    #[error("Memfow driver initialization requires a connector parameter")]
    MissingConnectorParameter,
    #[error("Invalid format for Memflow connector argument (key=value), got {0}")]
    InvalidConnectorArgument(String),
}

pub struct Memflow;

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
        unsafe {
            inventory
                .create_connector(&memflow_init_params.connector_name, &create_connector_args)?
        };
        Ok(Memflow {})
    }
}

impl Introspectable for Memflow {
    fn get_driver_type(&self) -> DriverType {
        DriverType::Memflow
    }
}
