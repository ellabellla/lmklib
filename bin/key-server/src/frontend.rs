use std::{collections::HashSet, hash::Hash, sync::Arc};

use async_trait::async_trait;
use serde::{Serialize, Deserialize};

use crate::modules::ModuleManager;


#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
/// Function controller configuration data types, used for serialization
pub enum FrontendConfigData {
    CommandPool,
    HID {
        mouse: String,
        keyboard: String,
        led: String,
    },
    MidiController,
    NanoMsg {
        pub_addr: String,
        sub_addr: String,
        timeout: i64,
    },
    RPC {
        front: String,
        back: String,
    }
}

impl Hash for FrontendConfigData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl PartialEq for FrontendConfigData {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

#[async_trait]
/// Function config interface, used to serialize function controller data
pub trait FrontendConfig {
    type Output;
    type Error;
    fn to_config_data(&self) -> FrontendConfigData;
    async fn from_config(
        function_config: &FrontendConfiguration,
    ) -> Result<Self::Output, Self::Error>;
}

/// Function configuration, managers function controller configs
pub struct FrontendConfiguration {
    pub module_manager: Arc<ModuleManager>,
    configs: HashSet<FrontendConfigData>,
}

impl FrontendConfiguration {
    /// New
    pub fn new(
        config: &str,
        module_manager: Arc<ModuleManager>,
    ) -> Result<FrontendConfiguration, serde_json::Error> {
        let configs = serde_json::from_str(config)?;
        Ok(FrontendConfiguration {
            configs,
            module_manager,
        })
    }

    /// Create new config data
    pub fn create_config() -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&HashSet::<FrontendConfigData>::new())
    }

    #[allow(dead_code)]
    /// Insert configuration
    pub fn insert(&mut self, config: FrontendConfigData) -> bool {
        self.configs.insert(config)
    }

    /// Get first configuration where matches returns true
    pub fn get<M>(&self, matches: M) -> Option<&FrontendConfigData>
    where
        M: FnMut(&&FrontendConfigData) -> bool,
    {
        self.configs.iter().find(matches)
    }
}