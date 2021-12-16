use std::iter;

use crate::State;

pub fn render_loop(state: &mut State) -> Result<(), wgpu::SurfaceError> {
    let output = state.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    encoder.push_debug_group("Lil Thing");
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: if state.switch_board.read().space {
                            0.
                        } else {
                            1.
                        },
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&state.render_pipeline);
        //pass.set_bind_group(1, &self.entity_bind_group, &[entity.uniform_offset]);
        render_pass.set_index_buffer(state.index_buf.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.set_vertex_buffer(0, state.vertex_buf.slice(..));
        //render_pass.draw(0..3, 0..1);
        render_pass.draw_indexed(0..state.index_count as u32, 0, 0..1);
    }
    encoder.pop_debug_group();

    //     rpass.push_debug_group("Prepare data for draw.");
    //     rpass.set_pipeline(&self.pipeline);
    //     rpass.set_bind_group(0, &self.bind_group, &[]);
    //     rpass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint16);
    //     rpass.set_vertex_buffer(0, self.vertex_buf.slice(..));
    //     rpass.pop_debug_group();
    //     rpass.insert_debug_marker("Draw!");
    //     rpass.draw_indexed(0..self.index_count as u32, 0, 0..1);
    //     if let Some(ref pipe) = self.pipeline_wire {
    //         rpass.set_pipeline(pipe);
    //         rpass.draw_indexed(0..self.index_count as u32, 0, 0..1);
    //     }
    // }

    state.queue.submit(iter::once(encoder.finish()));
    output.present();

    Ok(())
}
