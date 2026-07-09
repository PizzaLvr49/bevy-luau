use crate::fields::LuauFieldType;
use bevy::{ecs::component::ComponentId, platform::collections::HashMap, prelude::Resource};
use lasso::Spur;
use smallvec::SmallVec;

/// Registry to hold schemas
#[derive(Resource, Default)]
pub struct SchemaRegistry {
    fields: HashMap<ComponentId, SmallVec<[(Spur, usize, LuauFieldType); 6]>>,
}

impl SchemaRegistry {
    pub(crate) fn insert(
        &mut self,
        id: ComponentId,
        offsets: SmallVec<[(Spur, usize, LuauFieldType); 6]>,
    ) {
        self.fields.insert(id, offsets);
    }
    pub(crate) fn get(&self, id: ComponentId) -> &[(Spur, usize, LuauFieldType)] {
        self.fields.get(&id).map_or(&[], |v| &v[..])
    }
}
