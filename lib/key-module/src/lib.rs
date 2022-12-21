#![doc = include_str!("../README.md")]

use abi_stable::{StableAbi, std_types::RString};
use serde::{Serialize, Deserialize};

pub mod function;
pub mod driver;


#[repr(C)]
#[derive(StableAbi,  Debug, Clone, Serialize, Deserialize)]
#[sabi(impl_InterfaceType(Sync, Send, Debug, Display))]
/// Config data passed from the key server to the module
pub struct Data {
    /// Type name
    pub name: RString,
    /// User config data
    pub data: RString,
}