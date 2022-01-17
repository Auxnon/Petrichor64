use std::{f32::consts::PI, iter};

use cgmath::SquareMatrix;

use crate::{
    ent::{self, Ent, EntityUniforms},
    lua_define, State,
};

pub fn generate_matrix(
    aspect_ratio: f32,
    rot: f32,
) -> (cgmath::Matrix4<f32>, cgmath::Matrix4<f32>) {
    let mx_projection = cgmath::perspective(cgmath::Deg(45f32), aspect_ratio, 1., 800.0);
    let mx_view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::new(128. * rot.cos(), 128. * rot.sin(), 114.0),
        cgmath::Point3::new(0f32, 0.0, 0.0),
        cgmath::Vector3::unit_z(),
    );
    (mx_view, mx_projection)
}

pub fn render_loop(state: &mut State) -> Result<(), wgpu::SurfaceError> {
    let output = state.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    state.gui.render(&state.device, &state.queue, state.value2);

    state.value += 0.002;
    if state.value > 1. {
        state.value = 0.;
    }
    state.value2 += 0.02;
    if state.value2 > 1. {
        state.value2 = 0.;
        let typeOf = state.entities.len() % 2 == 0;
        state.entities.push(Ent::new(
            cgmath::vec3(
                (2. * state.entities.len() as f32) - 100.,
                if typeOf { 0. } else { -9. },
                0.,
            ),
            0.,
            if typeOf { 1. } else { 1. / 8. },
            0.,
            if typeOf {
                "chicken".to_string()
            } else {
                "package".to_string()
            },
            if typeOf {
                "plane".to_string()
            } else {
                "package".to_string()
            },
            (state.entities.len() as u64 * state.uniform_alignment) as u32,
            typeOf,
            Some("walker".to_string()),
        ))
    }

    let (mx_view, mx_persp) = generate_matrix(
        state.size.width as f32 / state.size.height as f32,
        state.value * 2. * std::f32::consts::PI,
    );

    // let rot = cgmath::Matrix4::from_angle_y(a);
    // //let mx_ref: = mx_total.as_ref();
    // let mx_totals = rot * state.camera_matrix;
    let mx_view_ref: &[f32; 16] = mx_view.as_ref();
    let mx_persp_ref: &[f32; 16] = mx_persp.as_ref();

    let time_ref: [f32; 4] = ([
        state.value,
        0.,
        state.size.width as f32,
        state.size.height as f32,
    ]);

    state
        .queue
        .write_buffer(&state.uniform_buf, 0, bytemuck::cast_slice(mx_view_ref));
    state
        .queue
        .write_buffer(&state.uniform_buf, 64, bytemuck::cast_slice(mx_persp_ref));

    //TODO should use offset of mat4 buffer size, 64 by deffault, is it always?
    state
        .queue
        .write_buffer(&state.uniform_buf, 128, bytemuck::cast_slice(&time_ref));

    let m: cgmath::Matrix4<f32> = cgmath::Matrix4::identity();
    let data = EntityUniforms {
        model: m.into(),
        color: [1., 1., 1., 1.],
        uv_mod: [0., 0., 1., 1.],
        effects: [0, 0, 0, 0],
    };
    //println!("model {} pos {} {}", i, entity.tex.x, entity.tex.y);
    state.queue.write_buffer(
        &state.entity_uniform_buf,
        0 as wgpu::BufferAddress,
        bytemuck::bytes_of(&data),
    );

    for (i, entity) in &mut state.entities.iter_mut().enumerate() {
        entity.run();

        //cgmath::Matrix4::from_angle_x(theta)
        let rotation = cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation));
        //entity.rotation += 0.1;
        //entity.rotation %= std::f32::consts::PI * 2.;
        //entity.pos.x += 1.;

        let transform = cgmath::Decomposed {
            disp: entity.pos,
            rot: cgmath::Quaternion::from_sv(entity.rotation, cgmath::Vector3::new(0., 0., 1.)),
            //rot: cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation)),
            scale: entity.scale,
        };

        // let pos = cgmath::Matrix4::from_translation(entity.pos);
        entity.matrix = cgmath::Matrix4::from(transform);

        let data = EntityUniforms {
            model: entity.matrix.into(),
            color: [
                entity.color.r as f32,
                entity.color.g as f32,
                entity.color.b as f32,
                entity.color.a as f32,
            ],
            uv_mod: [entity.tex.x, entity.tex.y, entity.tex.z, entity.tex.w],
            effects: [
                entity.effects.x,
                entity.effects.y,
                entity.effects.z,
                entity.effects.w,
            ],
        };
        //println!("model {} pos {} {}", i, entity.tex.x, entity.tex.y);
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

    encoder.push_debug_group("World Render");
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

        //world space
        {
            render_pass.set_pipeline(&state.render_pipeline);
            render_pass.set_bind_group(0, &state.bind_group, &[]);
            let c = state.world.get_tile_mut(0, 0);
            if c.buffers.is_some() {
                let b = c.buffers.as_ref().unwrap();
                render_pass.set_bind_group(1, &state.entity_bind_group, &[0]);
                render_pass.set_index_buffer(b.1.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.set_vertex_buffer(0, b.0.slice(..));
                render_pass.draw_indexed(0..c.ind_data.len() as u32, 0, 0..1);
            }

            for entity in &state.entities {
                let model = entity.model.get().unwrap();
                render_pass.set_bind_group(1, &state.entity_bind_group, &[entity.uniform_offset]);
                render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);
                render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
                render_pass.draw_indexed(0..model.index_count as u32, 0, 0..1);
            }
        }

        //gui space
        {
            // let res = state.gui.model.get();
            // if res.is_some() {
            //     let model = res.unwrap();
            render_pass.set_pipeline(&state.gui.gui_pipeline);
            render_pass.set_bind_group(0, &state.gui.gui_group, &[]);
            render_pass.draw(0..4, 0..4);
            //render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);
            //render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
            //render_pass.draw_indexed(0..model.index_count as u32, 0, 0..1);
            //}
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

pub fn log(str: String) {
    crate::log::log(format!("🖌render::{}", str));
}
