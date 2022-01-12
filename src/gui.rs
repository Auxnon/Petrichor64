pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
}
impl Gui {
    pub fn new(gui_pipeline: wgpu::RenderPipeline, gui_group: wgpu::BindGroup) -> Gui {
        Gui {
            gui_pipeline,
            gui_group,
        }
    }
}
pub fn draw() {}
