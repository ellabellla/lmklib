use std::{path::Path};

use abi_stable::{StableAbi, std_types::{RString, RBox, RVec, RResult}, library::{RootModule, LibraryError}, sabi_types::VersionStrings, package_version_strings, sabi_trait};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = DriverModuleRef)))]
#[sabi(missing_field(panic))]
/// Driver module interface
pub struct DriverModule {
    #[sabi(last_prefix_field)]
    /// Initialize driver interface
    pub new_driver: extern "C" fn() -> DriverBox,
}

impl RootModule for DriverModuleRef {
    abi_stable::declare_root_module_statics! {DriverModuleRef}

    const BASE_NAME: &'static str = "driver_module";
    const NAME: &'static str = "driver_module";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[sabi_trait]
#[sabi(impl_InterfaceType(Sync, Send, Debug, Display))]
/// Driver interface
pub trait Driver {
    /// Initialize new driver from key server config data
    /// Returns the id of the new driver
    fn load_data<'borr, A>(&mut self, data: RString) -> abi_stable::std_types::RResult<u64,RString>;

    /// Poll the current state of the driver with the specified id
    fn poll(&mut self, id: u64) -> RResult<RVec<u16>, RString>;

    #[sabi(last_prefix_field)]
    //. Set the current state of the driver with the specified id
    fn set(&mut self, id: u64, idx: usize, state: u16) -> RResult<(), RString>;
}

pub type DriverBox = Driver_TO<'static, RBox<()>>;

/// Load from file
pub fn load_module(path: &Path) -> Result<DriverModuleRef, LibraryError> {
    abi_stable::library::lib_header_from_path(path)
            .and_then(|x| x.init_root_module::<DriverModuleRef>())
}