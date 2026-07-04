use sea_orm::{Select, EntityTrait};

pub async fn test<E: EntityTrait>(sel: Select<E>) {
    // Try to see what methods are available
    let _x = sel.  // Try autocomplete
}
