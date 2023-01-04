use std::{path::Path};

use abi_stable::{StableAbi, std_types::{RString, RBox}, library::{RootModule, LibraryError}, sabi_types::VersionStrings, package_version_strings, sabi_trait};

use crate::Data;

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = FunctionModuleRef)))]
#[sabi(missing_field(panic))]
/// Function module interface
pub struct FunctionModule {
    #[sabi(last_prefix_field)]
    /// Initialize function interface
    pub new_function: extern "C" fn() -> FunctionBox,
}

impl RootModule for FunctionModuleRef {
    abi_stable::declare_root_module_statics! {FunctionModuleRef}

    const BASE_NAME: &'static str = "function_module";
    const NAME: &'static str = "function_module";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[sabi_trait]
#[sabi(impl_InterfaceType(Sync, Send, Debug, Display))]
/// Function driver
pub trait Function {
    /// Initialize new function from key server config data
    /// Returns the id of the new driver
    fn load_data<'borr, A>(&mut self, data: Data) -> abi_stable::std_types::RResult<u64,RString>;

    #[sabi(last_prefix_field)]
    /// Keyboard pool event, runs every time the keyboard polls the state associated with the function
    fn event(&mut self, id: u64, state: u16) -> abi_stable::std_types::RResult<(), RString>;
}

pub type FunctionBox = Function_TO<'static, RBox<()>>;

/// Load from file
pub fn load_module(path: &Path) -> Result<FunctionModuleRef, LibraryError> {
    abi_stable::library::lib_header_from_path(path)
            .and_then(|x| x.init_root_module::<FunctionModuleRef>())
}