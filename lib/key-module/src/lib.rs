use abi_stable::{StableAbi, std_types::RString};
use serde::{Serialize, Deserialize};

pub mod function;
pub mod driver;


#[repr(C)]
#[derive(StableAbi,  Debug, Clone, Serialize, Deserialize)]
#[sabi(impl_InterfaceType(Sync, Send, Debug, Display))]
pub struct Data {
    pub name: RString,
    pub data: RString,
}