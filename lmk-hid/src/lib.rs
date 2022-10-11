pub enum Modifier {
    RightMeta,
    RightAlt,
    RightShift,
    RightControl,
    LeftMeta,
    LeftAlt,
    LeftShift,
    LeftControl,
}

impl Modifier {
    pub fn all_to_byte(modifiers: Vec<Modifier>) -> u8 {
        modifiers.iter()
            .map(|modi| modi.to_byte())
            .reduce(|accum, byte| accum | byte)
            .unwrap_or(0)
    }

    pub fn to_byte(&self) -> u8 {
        let base = 0b00000001;
        match self {
            Modifier::RightMeta => 0b00000001 << 7,
            Modifier::RightAlt => 0b00000001 << 6,
            Modifier::RightShift => 0b00000001 << 5,
            Modifier::RightControl => base << 4,
            Modifier::LeftMeta => base << 3,
            Modifier::LeftAlt => base << 2,
            Modifier::LeftShift => base << 1,
            Modifier::LeftControl => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers() {
        println!("{:#8b}", Modifier::LeftMeta.to_byte());
    }
}

