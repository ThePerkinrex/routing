use bevy::prelude::Entity;
pub mod link;
pub mod physical;

#[derive(Debug)]
pub struct EntityEvent<T>(pub Entity, pub T);
