pub struct SwitchBoard {
    pub space: bool,
    pub h: f32,
}
impl SwitchBoard {
    pub fn new() -> SwitchBoard {
        SwitchBoard {
            space: false,
            h: 0f32,
        }
    }
}
