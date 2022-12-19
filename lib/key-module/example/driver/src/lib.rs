use abi_stable::{export_root_module, sabi_extern_fn, sabi_trait::TD_Opaque, std_types::{RString, RResult::{self, ROk, RErr}, RVec}, prefix_type::PrefixTypeTrait};
use key_module::{driver::{DriverModuleRef, DriverModule, DriverBox, Driver}, Data};
use slab::Slab;
use serde::{Serialize, Deserialize};

#[export_root_module]
pub fn get_library() -> DriverModuleRef {
    DriverModule {
        new_driver    
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn new_driver() -> DriverBox {
    DriverBox::from_value(DriverManager{drivers: Slab::new()}, TD_Opaque)
}


#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConstData {
    name: String,
    state: RVec<u16>,
}

pub struct DriverManager {
    drivers: Slab<ConstData>
}

impl Driver for DriverManager {
    fn load_data(&mut self, data: Data) -> RResult<u64, RString> {
        if data.name == "Const" {
            let data = match serde_json::from_str(&data.data) {
                Ok(data) => data,
                Err(e) => return RErr(format!("{}", e).into())
            };
            let id = self.drivers.insert(data) as u64;

            return ROk(id)
        }
        return RErr("Unknown function".into())
    }

    fn name(&self, id: u64) -> RResult<RString, RString> {
        if let Some(name) = self.drivers.get(id as usize)
            .map(|data| RString::from(data.name.to_string())) {
                ROk(name.into()) 
        } else {
            RErr(RString::from("Driver not found"))
        }
    }

    fn poll(&mut self, id:u64) -> RResult<RVec<u16>, RString> {
        if let Some(data) = self.drivers.get(id as usize) {
            ROk(data.state.clone()) 
        } else {
            RErr(RString::from("Driver not found"))
        }
    }
}