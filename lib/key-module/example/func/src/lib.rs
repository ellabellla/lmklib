
use abi_stable::{std_types::{RResult::{self, ROk, RErr}, RString}, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, sabi_trait::TD_Opaque};
use key_module::{function::{Function, FunctionModuleRef, FunctionModule, FunctionBox}, Data};
use slab::Slab;

#[export_root_module]
pub fn get_library() -> FunctionModuleRef {
    FunctionModule {
        new_function    
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn new_function() -> FunctionBox {
    FunctionBox::from_value(FunctionManager{funcs: Slab::new()}, TD_Opaque)
}

pub struct FunctionManager {
    funcs: Slab<(RString, u16)>
}

impl Function for FunctionManager {
    fn load_data(&mut self, data: Data) -> RResult<u64, RString> {
        if data.name == "Print" {
            let id = self.funcs.insert((data.data, 0)) as u64;

            return ROk(id)
        }
        return RErr("Unknown function".into())
    }

    fn event(&mut self, id: u64, state: u16) -> RResult<(),RString> {
        if let Some((data, prev_state)) = self.funcs.get_mut(id as usize) {
            if state == 1 && *prev_state == 0 {
                println!("Print: {}", data);
            }

            *prev_state = state;
            ROk(())
        } else {
            RErr("No such function".into())
        }
        
    }
}