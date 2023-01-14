use std::{path::Path};

use abi_stable::{StableAbi, std_types::{RString, RBox}, library::{RootModule, LibraryError}, sabi_types::VersionStrings, package_version_strings, sabi_trait};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = HidModuleRef)))]
#[sabi(missing_field(panic))]
/// HID module interface
pub struct HidModule {
    #[sabi(last_prefix_field)]
    /// Initialize HID interface
    pub new_hid: extern "C" fn() -> HIDBox,
}

impl RootModule for HidModuleRef {
    abi_stable::declare_root_module_statics! {HidModuleRef}

    const BASE_NAME: &'static str = "hid_module";
    const NAME: &'static str = "hid_module";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

#[sabi_trait]
#[sabi(impl_InterfaceType(Sync, Send, Debug, Display))]
/// HID interface
pub trait HID {
    fn hold_key(&mut self, key: usize);
    fn hold_special(&mut self, special: usize);
    fn hold_modifier(&mut self, modifier: usize);
    fn release_key(&mut self, key: usize);
    fn release_special(&mut self, special: usize);
    fn release_modifier(&mut self, modifier: usize);
    fn press_basic_str(&mut self, str: RString);
    fn press_str(&mut self, layout: RString, str: RString);
    fn scroll_wheel(&mut self, amount: i8);
    fn move_mouse_x(&mut self, amount: i8);
    fn move_mouse_y(&mut self, amount: i8);
    fn hold_button(&mut self, button: usize);
    fn release_button(&mut self, button: usize);
    fn send_keyboard(&mut self);
    fn send_mouse(&mut self);
}

pub type HIDBox = HID_TO<'static, RBox<()>>;

/// Load from file
pub fn load_module(path: &Path) -> Result<HidModuleRef, LibraryError> {
    abi_stable::library::lib_header_from_path(path)
            .and_then(|x| x.init_root_module::<HidModuleRef>())
}