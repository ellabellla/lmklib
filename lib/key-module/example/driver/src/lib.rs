use abi_stable::{export_root_module, sabi_extern_fn, sabi_trait::TD_Opaque, std_types::{RString, RResult::{self, ROk, RErr}, RVec}, prefix_type::PrefixTypeTrait};
use key_module::{driver::{DriverModuleRef, DriverModule, DriverBox, Driver}};
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
    state: RVec<u16>,
}

pub struct DriverManager {
    drivers: Slab<ConstData>
}

impl Driver for DriverManager {
    fn load_data(&mut self, data: RString) -> RResult<u64, RString> {
        let data = match serde_json::from_str(&data) {
            Ok(data) => data,
            Err(e) => return RErr(format!("{}", e).into())
        };
        let id = self.drivers.insert(data) as u64;

        return ROk(id);
    }

    fn poll(&mut self, id:u64) -> RResult<RVec<u16>, RString> {
        if let Some(data) = self.drivers.get(id as usize) {
            ROk(data.state.clone()) 
        } else {
            RErr(RString::from("Driver not found"))
        }
    }

    fn set(&mut self, id:u64, idx: usize, state:u16) -> RResult<(), RString> {
        if let Some(data) = self.drivers.get_mut(id as usize) {
            if let Some(curr_state) = data.state.get_mut(idx) {
                *curr_state = state;
                ROk(())
            } else {
                RErr(RString::from("Idx out of bounds"))
            }
        } else {
            RErr(RString::from("Driver not found"))
        }
    }
}