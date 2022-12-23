pub enum ValueMap {
    String(String),
    Integer(i32),
    Float(f32),
    Bool(bool),
    // Table(Vec<(String, ValueMap)>),
    Array(Vec<ValueMap>),
    // Map(std::collections::HashMap<String, ValueMap>),
    Null(),
}
