use std::{ops::Range, fmt::Display, sync::Arc};

use itertools::Itertools;
use serde::{Serialize, Deserialize, de};
use slab::Slab;
use tokio::sync::RwLock;

use crate::{function::{Function, ReturnCommand, FunctionType, FunctionBuilder}, driver::DriverManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Driver state address. Used to index a state/s of a driver
enum Address {
    DriverMatrix {
        name: String,
        input: Range<usize>,
        width: usize,
        root: (usize, usize),
    },
    DriverAddr {
        name: String,
        input: usize,
        root: (usize, usize),
    },
    None,
}

#[derive(Debug)]
/// Layout error
pub enum LayoutError {
    /// Tried to bind an address outside the bounds of a layout
    OutsideBounds,
    /// Tried to bind an address to an area of layout already bound
    InUse,
    /// Address Matrix was of an invalid size
    InvalidSize,
}

impl Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::OutsideBounds => f.write_str("Binding outside bounds of layout"),
            LayoutError::InUse => f.write_str("Section already in use"),
            LayoutError::InvalidSize => f.write_str("Binding is an invalid size"),
        }
    }
}

pub struct LayoutBuilder {
    width: usize,
    height: usize,
    none: usize,
    addresses: Slab<Address>,
    layout: Vec<usize>,
    layers: Vec<Vec<FunctionType>>
}

impl LayoutBuilder {
    /// New
    pub fn new( width: usize, height: usize) -> LayoutBuilder {
        let mut addresses =  Slab::new();
        let none = addresses.insert(Address::None);
        LayoutBuilder { width, height, none, addresses, layout: vec![none; width * height], layers: vec![] }
    }

    /// Add bind point
    pub fn add_point(&mut self, name: &str, idx: usize, location: (usize, usize)) -> Result<(), LayoutError> {
        let (x, y) = location;

        if x >= self.width || y >= self.height {
            return Err(LayoutError::OutsideBounds)
        }

        let i = x + y * self.width;

        if self.layout[i] != self.none {
            return Err(LayoutError::InUse);
        }

        let id = self.addresses.insert(Address::DriverAddr { name: name.to_string(), input: idx, root: location });
        self.layout[i] = id;
        Ok(())
    }

    /// Add a matrix of bind points
    pub fn add_matrix(&mut self, name: &str, range: Range<usize>, width: usize, location: (usize, usize)) -> Result<(), LayoutError> {
        let (x, y) = location;
        let size = range.len();
        let height =  size / width;

        if size % width != 0 {
            return Err(LayoutError::InvalidSize);
        }

        if x >= self.width || y >= self.height || x + width > self.width || y + height > self.height {
            return Err(LayoutError::OutsideBounds)
        }

        let mut i = x + y * self.width;
        
        for _ in 0..height {
            for _ in 0..width {
                if self.layout[i] != self.none {
                    return Err(LayoutError::InUse);
                }
                i += 1;
            }
            i -= width;
            i += self.width;
        }

        let id = self.addresses.insert(Address::DriverMatrix { name: name.to_string(), input: range, width: width, root: location });

        let mut i = x + y * self.width;
        
        for _ in 0..height {
            for _ in 0..width {
                self.layout[i] = id;
                i += 1;
            }
            i -= width;
            i += self.width;
        }

        Ok(())
    }

    /// Build layout
    pub async fn build(self, driver_manager: Arc<RwLock<DriverManager>>, function_builder: Arc<RwLock<FunctionBuilder>>) -> Arc<RwLock<Layout>> {
        let mut layer_stack = Vec::new();
        for layer in self.layers.into_iter() {
            let mut built_layer = Vec::new();
            for entry in layer.into_iter() {
                built_layer.push(function_builder.read().await.build(entry).await)
            }
            layer_stack.push(built_layer);
        }
        Arc::new(RwLock::new(Layout { 
            width: self.width, 
            height: self.height, 
            addresses: self.addresses, 
            driver_manager: driver_manager, 
            function_builder: function_builder,
            layer_stack,
            cur_layer: 0,
            shift_from: vec![],
        }))
    }
}


impl Serialize for LayoutBuilder {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        #[derive(Serialize)]
        struct Layout<'a> {
            width: usize,
            height: usize,
            bound: Vec<&'a Address>,
            layers: Vec<Vec<Vec<FunctionType>>>,
        }
        let layers: Vec<Vec<Vec<FunctionType>>> = self.layers.iter()
            .map(|layer| {
                layer.clone()
                .into_iter()
                .collect::<Vec<FunctionType>>()
                .chunks(self.width)
                .map(|a| a.to_vec())
                .collect::<Vec<Vec<FunctionType>>>()
        }).collect();
        let bound = self.addresses.iter().map(|(_, a)| a).collect::<Vec<&Address>>();    
        Layout{width: self.width, height: self.height, bound, layers: layers}.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LayoutBuilder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        #[derive(Deserialize)]
        struct Layout {
            width: usize,
            height: usize,
            bound: Vec<Address>,
            layers: Vec<Vec<Vec<FunctionType>>>,
        }
        let layout = Layout::deserialize(deserializer)?;
        let mut builder = LayoutBuilder::new(layout.width, layout.height);

        for (i, address) in layout.bound.into_iter().enumerate() {
           match address {
                Address::DriverMatrix { name, input, width, root } => 
                    if let Err(e) = builder.add_matrix(&name, input, width, root) {
                        return Err(de::Error::custom(format!("Error adding bound address at {}, {}", i, e)))
                    },
                Address::DriverAddr { name, input, root } => 
                if let Err(e) = builder.add_point(&name, input, root) {
                    return Err(de::Error::custom(format!("Error adding bound address at {}, {}", i, e)))
                },
                Address::None => continue,
            }
        }
        for (i, layer) in layout.layers.into_iter().enumerate() {
            let mut new_layer: Vec<FunctionType> = vec![];
            if layer.len() != layout.height {
                return Err(de::Error::custom(format!("Layer {} should have the same height as the layout.", i)))
            }
            for (y, row) in layer.into_iter().enumerate() {
                if row.len() != layout.width {
                    return Err(de::Error::custom(format!("Layer {}, row {}, should have the same width as the layout.", i, y)))
                }
                new_layer.extend(row);
            }
            builder.layers.push(new_layer);
        }

        Ok(builder)
    }
}


/// Layout
pub struct Layout {
    width: usize,
    height: usize,
    addresses: Slab<Address>,

    driver_manager: Arc<RwLock<DriverManager>>,
    function_builder: Arc<RwLock<FunctionBuilder>>,

    layer_stack: Vec<Vec<Function>>,
    cur_layer: usize,

    shift_from: Vec<(usize, usize)>,
}

impl Layout {
    /// Switch layout
    pub fn switch_layer(&mut self,  index: usize) -> Option<()> {
        if index >= self.layer_stack.len() {
            None
        } else {
            self.cur_layer = index;
            Some(())
        }
    }

    /// Move one layer above
    pub fn up_layer(&mut self) -> Option<()> {
        if self.cur_layer + 1 >= self.layer_stack.len() {
            None
        } else {
            self.cur_layer += 1;
            Some(())
        }
    }

    /// Move one layer down
    pub fn down_layer(&mut self) -> Option<()> {
        if self.cur_layer - 1 >= self.layer_stack.len() {
            None
        } else {
            self.cur_layer -= 1;
            Some(())
        }
    }


    pub fn shift(&mut self, index: usize) -> Option<()> {
        if index >= self.layer_stack.len() || self.cur_layer == index  || self.shift_from.len() == 0 {
            None
        } else {
            self.shift_from.push((self.cur_layer, index));
            self.cur_layer = index;
            Some(())
        }
    }

    pub fn unshift(&mut self, index: usize) -> Option<()> {
        if index >= self.layer_stack.len() || self.shift_from.len() == 0  {
            None
        } else {
            if let Some((start, curr)) = self.shift_from.last() {
                if *curr == index {
                    self.switch_layer(*start);
                    self.shift_from.pop();
                    return Some(())
                }
            }
            let mut i = self.shift_from.len() -1;
            loop {
                let (_, curr) = self.shift_from[i];
                if curr == index {
                    self.shift_from.remove(i);
                    return Some(())
                }
                if i == 0 {
                    break;
                }
                i -= 1;
            }
            None
        }
    }

    /// Remove layer
    pub fn remove_layer(&mut self, index: usize) -> Option<Vec<Function>> {
        if index >= self.layer_stack.len() {
            return None;
        }

        let layer = self.layer_stack.remove(index);

        Some(layer)
    }

    /// Number of layers
    pub fn num_layers(&self) -> usize {
        self.layer_stack.len()
    }

    /// Remove layer
    pub fn curr_layer(&self) -> usize {
        self.cur_layer
    }

    /// Number of layers
    pub fn layer_len(&self) -> usize {
        self.layer_stack.len()
    }

    /// Add layer
    pub async fn add_layer(&mut self, layer: Vec<Vec<FunctionType>>, index: usize) -> Result<usize, LayoutError> {
        if layer.len() > self.height {
            return Err(LayoutError::InvalidSize)
        }

        let mut built_layer = Vec::with_capacity(self.width*self.height);

        let function_builder = self.function_builder.read().await;

        for row in layer {
            if row.len() > self.width {
                return Err(LayoutError::InvalidSize)
            }
            for ftype in row {
                built_layer.push(function_builder.build(ftype).await)
            }
        }

        if index > self.layer_stack.len() {
            self.layer_stack.push(built_layer);
            Ok(self.layer_stack.len())
        } else {
            self.layer_stack.insert(index, built_layer);
            Ok(index)
        }
    }

    /// Tick drivers
    pub async fn tick(&mut self) {
        self.driver_manager.write().await.tick().await;
    }

    /// Get string representing layout
    pub fn layout_string(&self) -> Option<String> {
        serde_json::to_string(
            &self.layer_stack[self.cur_layer].iter()
                .map(|func| {
                    func.as_ref().map(|func| func.ftype())
                })
                .chunks(self.width)
                .into_iter()
                .map(|chunk| chunk.collect_vec())
                .collect_vec()
        ).ok()
    }

    /// Poll the layout states and call corresponding functions
    pub async fn poll(&mut self) {
        if self.layer_stack.len() == 0 {
            return;
        }
        
        let mut commands = vec![];

        for (_, address) in self.addresses.iter_mut() {
            match address {
                Address::DriverMatrix { name, input, width, root} => {
                    let driver_manager = self.driver_manager.read().await;
                    let Some(driver) = driver_manager.get(name) else {
                        continue;
                    };

                    let Some(state) = driver.poll_range(input) else {
                        continue;
                    };
                    
                    let state = state.to_vec();

                    drop(driver);
                    drop(driver_manager);

                    let (mut x, mut y) = root;

                    for state in state {
                        for layer in self.layer_stack[..self.cur_layer+1].iter_mut().rev() {
                            match &mut layer[x + (y * self.width)] {
                                Some(func) => {
                                    let res = func.event(state).await;
                                    if !matches!(res, ReturnCommand::None) {
                                        commands.push(res);
                                    }
                                    break;
                                },
                                None=> {
                                    continue;
                                },
                            }
                        }
                        
                        x += 1;
                        if x % *width == 0 {
                            x = 0;
                            y += 1;
                        }
                    }
                },
                Address::DriverAddr { name, input, root} => {
                    let driver_manager = self.driver_manager.read().await;
                    let Some(driver) = driver_manager.get(name) else {
                        continue;
                    };

                    let (x, y) = root;

                    let state = driver.poll(*input);

                    drop(driver);
                    drop(driver_manager);

                    for layer in self.layer_stack[..self.cur_layer+1].iter_mut().rev() {
                        match &mut layer[*x + (*y * self.width)] {
                            Some(func) => {
                                let res = func.event(state).await;
                                if !matches!(res, ReturnCommand::None) {
                                    commands.push(res);
                                }
                                break;
                            },
                            None=> {
                                continue;
                            },
                        }
                    }
                },
                Address::None => continue,
            }
        }

        for command in commands {
            command.eval(self);
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self)
    }
}

impl Serialize for Layout {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        #[derive(Serialize)]
        struct Layout<'a> {
            width: usize,
            height: usize,
            bound: Vec<&'a Address>,
            layers: Vec<Vec<Vec<FunctionType>>>
        }
        let layers: Vec<Vec<Vec<FunctionType>>> = self.layer_stack.iter()
            .map(|layer| {
                layer.iter()
                .map(|func| FunctionType::from_function(func))
                .collect::<Vec<FunctionType>>()
                .chunks(self.width)
                .map(|a| a.to_vec())
                .collect::<Vec<Vec<FunctionType>>>()
        }).collect();
        let bound = self.addresses.iter().map(|(_, a)| a).collect::<Vec<&Address>>();    
        Layout{width: self.width, height: self.height, bound, layers: layers}.serialize(serializer)
    }
}