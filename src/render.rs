use std::iter;

use crate::{ent::EntityUniforms, State};

pub fn render_loop(state: &mut State) -> Result<(), wgpu::SurfaceError> {
    let output = state.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    for entity in &mut state.entities {
        if entity.rotation_speed != 0.0 {
            let rotation = cgmath::Matrix4::from_angle_x(cgmath::Deg(entity.rotation_speed));
            entity.pos.x += 0.1;
            let pos = cgmath::Matrix4::from_translation(entity.pos);
            entity.matrix = entity.matrix * rotation * pos;
        }
        let data = EntityUniforms {
            model: entity.matrix.into(),
            color: [
                entity.color.r as f32,
                entity.color.g as f32,
                entity.color.b as f32,
                entity.color.a as f32,
            ],
        };
        state.queue.write_buffer(
            &state.entity_uniform_buf,
            entity.uniform_offset as wgpu::BufferAddress,
            bytemuck::bytes_of(&data),
        );
    }

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
        render_pass.set_bind_group(0, &state.bind_group, &[]);
        //entity.uniform_offset
        // render_pass.set_index_buffer(state.index_buf.slice(..), wgpu::IndexFormat::Uint16);
        // render_pass.set_vertex_buffer(0, state.vertex_buf.slice(..));

        for entity in &state.entities {
            render_pass.set_bind_group(1, &state.entity_bind_group, &[entity.uniform_offset]);
            render_pass.set_index_buffer(entity.index_buf.slice(..), entity.index_format);
            render_pass.set_vertex_buffer(0, entity.vertex_buf.slice(..));
            render_pass.draw_indexed(0..entity.index_count as u32, 0, 0..1);
        }

        //render_pass.draw(0..3, 0..1);
        //render_pass.draw_indexed(0..state.index_count as u32, 0, 0..1);
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

    // queue.write_buffer(
    //     &self.forward_pass.uniform_buf,
    //     0,
    //     bytemuck::cast_slice(mx_ref),
    // );

    // queue.write_buffer(
    //     &self.entity_uniform_buf,
    //     entity.uniform_offset as wgpu::BufferAddress,
    //     bytemuck::bytes_of(&data),
    // );

    state.queue.submit(iter::once(encoder.finish()));
    output.present();

    Ok(())
}
