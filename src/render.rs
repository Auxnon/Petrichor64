use std::{f32::consts::PI, iter};

use crate::{ent::EntityUniforms, State};

fn generate_matrix(aspect_ratio: f32, rot: f32) -> cgmath::Matrix4<f32> {
    let mx_projection = cgmath::perspective(cgmath::Deg(45f32), aspect_ratio, 1.0, 40.0);
    let mx_view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::new(20. * rot.cos(), 20. * rot.sin(), 16.0),
        cgmath::Point3::new(0f32, 0.0, 0.0),
        cgmath::Vector3::unit_z(),
    );
    mx_projection * mx_view
}

pub fn render_loop(state: &mut State) -> Result<(), wgpu::SurfaceError> {
    let output = state.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    state.value += 0.002;
    if state.value > 1. {
        state.value = 0.;
    }

    let mx_totals = generate_matrix(
        state.size.width as f32 / state.size.height as f32,
        state.value * 2. * std::f32::consts::PI,
    );

    // let rot = cgmath::Matrix4::from_angle_y(a);
    // //let mx_ref: = mx_total.as_ref();
    // let mx_totals = rot * state.camera_matrix;
    let mx_ref: &[f32; 16] = mx_totals.as_ref();
    state
        .queue
        .write_buffer(&state.uniform_buf, 0, bytemuck::cast_slice(mx_ref));

    let time_ref: [f32; 4] = ([state.value, 0., 0., 0.]);

    //TODO should use offset of mat4 buffer size, 64 by deffault, is it always?
    state
        .queue
        .write_buffer(&state.uniform_buf, 64, bytemuck::cast_slice(&time_ref));

    for entity in &mut state.entities {
        if entity.rotation_speed != 0.0 {
            let rotation = cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation_speed));
            entity.rotation_speed += 0.1;
            entity.rotation_speed %= std::f32::consts::PI * 2.;

            //let pos = cgmath::Matrix4::from_translation(entity.pos);
            entity.matrix = entity.matrix * rotation;
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
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &state.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
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

    // queue.write_buffer(
    //     &self.entity_uniform_buf,
    //     entity.uniform_offset as wgpu::BufferAddress,
    //     bytemuck::bytes_of(&data),
    // );

    state.queue.submit(iter::once(encoder.finish()));
    output.present();

    Ok(())
}
