use std::{any::Any, sync::Arc, collections::{HashMap, hash_map::Keys}};

use serde::{Deserialize, Serialize};
use tokio::{sync::{RwLock, watch}};

use crate::OrLog;

pub struct Variables {
    pub data: HashMap<String, Vec<(watch::Sender<String>, watch::Receiver<String>)>>
}

impl Variables {
    pub fn new() -> Arc<RwLock<Variables>> {
        Arc::new(RwLock::new(Variables{ data: HashMap::new() }))
    }

    pub fn create_many(&mut self, variables: Vec<VarDef>) {
        for definition in variables {
            if !self.data.contains_key(&definition.name) {
                self.data.insert(definition.name, vec![watch::channel(definition.default)]);
            }else {
                self.data.get_mut(&definition.name).expect("Already check for keys existence").push(watch::channel(definition.default))
            }
        }
    }

    pub fn update(&self, name: &str, value: String) -> Option<()> {
        if let Some(watching) = self.data.get(name) {
            for (send, _) in watching {
                send.send_replace(value.to_string());
            }
            Some(())
        } else {
            None
        }
    }
    
    pub fn variables(&self) -> Keys<String, Vec<(watch::Sender<String>, watch::Receiver<String>)>>{
        self.data.keys()
    }

    pub fn get(&self, name: &str) -> Option<String> {
        self.data.get(name)
            .and_then(|watching| watching.first())
            .map(|(_, updates)| updates.borrow().to_string())
    }

    pub fn set(&mut self, name: &str, value: (watch::Sender<String>, watch::Receiver<String>)) {
        if !self.data.contains_key(name) {
            self.data.insert(name.to_string(), vec![value]);
        } else {
            if let Some(watching) = self.data.get_mut(name) {
                value.0.send(watching.first().expect("Should have atleast one entry").1.borrow().to_string())
                    .or_log("Unable to update variable watch channel");
                watching.push(value)
            }
        }
    }
}

#[derive(Clone)]
enum VariableData<T> 
where
    T: Any + Clone + Send + Sync + for<'a> Deserialize<'a>  + Serialize 
{
    Const(T),
    Var{name:String, data: T, updates: watch::Receiver<String>}
}

#[derive(Clone)]
pub struct Variable<T> 
where
    T: Any + Clone + Send + Sync + for<'a> Deserialize<'a>  + Serialize 
{
    data: VariableData<T>,
    variables: Arc<RwLock<Variables>>
}

impl<T> Variable<T>
where
    T: Any + Clone + Send + Sync + for<'a> Deserialize<'a> + Serialize,
{
    pub fn into_data(&self) -> Data<T> {
        match self.data.clone() {
            VariableData::Const(data) => Data::Const(data),
            VariableData::Var { name, data, updates:_ } => Data::VarDef { name, default: data },
        }
    }

    pub async fn from_data(data: Data<T>, default: T, variables: Arc<RwLock<Variables>>) -> Self 
    {   
        match data {
            Data::Const(data) => {
                Variable {data: VariableData::Const(data), variables }
            },
            Data::VarDef { name, default } => {
                let (send, mut updates) = watch::channel(serde_json::to_string(&default).unwrap_or_else(|_| "".to_string()));
                updates.borrow_and_update();
                 
                variables.write().await.set(&name, (send, updates.clone()));

                Variable {data: VariableData::Var { name, data: default, updates: updates }, variables }
            },
            Data::Var(name) => {
                let (send, mut updates) = watch::channel(serde_json::to_string(&default).unwrap_or_else(|_| "".to_string()));
                updates.borrow_and_update();
                variables.write().await.set(&name, (send, updates.clone()));

                let data = serde_json::from_str(&updates.borrow()).unwrap_or(default);

                Variable {data: VariableData::Var { name, data, updates: updates}, variables }
            },
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> Option<&String> {
        match &self.data {
            VariableData::Const(_) => None,
            VariableData::Var { name, data:_, updates:_ } => Some(name),
        }
    }

    pub fn data<'a>(&'a mut self) -> &'a T {
        match &mut self.data {
            VariableData::Var { name, data, updates } => {
                if updates.has_changed().unwrap_or(false) {
                    let new_data: Option<T> = serde_json::from_str(&updates.borrow_and_update())
                        .or_log(&format!("Unable to deserialize variable update (type issue?) (VAR {:?})", name));
                    if let Some(new_data) = new_data {
                        *data = new_data
                    }
                }
                data
            },
            VariableData::Const(data) => data,
        }
    }

    pub fn map<T2>(self, func: impl FnOnce(T) -> T2) -> Variable<T2>
    where
        T2: Any + Clone + Send + Sync + for<'a> Deserialize<'a> + Serialize ,
    {
        match self.data {
            VariableData::Const(data) => {
                let data = func(data);
                Variable {data: VariableData::Const(data), variables: self.variables.clone() }
            },
            VariableData::Var { name, data, updates } => {
                let data = func(data);
                Variable {data: VariableData::Var { name, data, updates }, variables: self.variables.clone() }
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Data<T> 
where
    T: Any + Clone + Send + Sync
{
    Const(T),
    Var(String),
    VarDef{name: String, default: T},
}

impl<T> Data<T> 
where
    T: Any + Clone + Send + Sync + for<'a> Deserialize<'a> + Serialize
{
    pub async fn into_variable(self, default: T, variables: Arc<RwLock<Variables>>) -> Variable<T> {
        Variable::from_data(self, default, variables).await
    }

    pub fn map<T2>(self, func: impl FnOnce(T) -> T2) -> Data<T2>
    where
        T2: Any + Clone + Send + Sync,
    {
        match self {
            Data::Const(data) => Data::Const(func(data)),
            Data::VarDef { name, default } => Data::VarDef { name, default: func(default) },
            Data::Var(name) => Data::Var(name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarDef {
    name: String, 
    default: String
}
