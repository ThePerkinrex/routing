use bevy::prelude::Component;

#[derive(Debug, Component)]
pub struct Interface {
    name: String,
}

impl Interface {
    pub const fn new(name: String) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
