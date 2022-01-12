use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::model::{get_model, Model};

pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
    pub model: Arc<OnceCell<Model>>,
}
impl Gui {
    pub fn new(gui_pipeline: wgpu::RenderPipeline, gui_group: wgpu::BindGroup) -> Gui {
        Gui {
            gui_pipeline,
            gui_group,
            model: get_model(&"plane".to_string()),
        }
    }
}
pub fn draw() {}
