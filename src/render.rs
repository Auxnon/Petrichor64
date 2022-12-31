use glam::{vec3, Mat4, Vec2, Vec3};
use std::{iter, ops::Add, rc::Rc};
// use tracy::frame;

use crate::{
    ent::{Ent, EntityUniforms},
    model::Model,
    Core,
};

/** create rotation matrix from camera position and simple rotation */
pub fn generate_matrix(aspect_ratio: f32, mut camera_pos: Vec3, mouse: Vec2) -> (Mat4, Mat4, Mat4) {
    let pi = std::f32::consts::PI;
    let mx_projection = Mat4::perspective_rh(0.785398, aspect_ratio, 1., 24800.0);

    camera_pos *= 16.;
    // println!("mouse {:?}", mouse);
    // let r = 0.5f32;

    // let mx_view = Mat4::look_at_rh(
    //     vec3(92. * r.cos(), -128., 82.0),
    //     vec3(0f32, 0.0, 0.0),
    //     Vec3::Z,
    // );

    // let mx_view = Mat4::IDENTITY;

    // let r = pi * (0.5 + (mouse.0 % 100.) / 50.);
    // let azimuth = pi * (0.5 + (mouse.1 % 100.) / 50.);
    let r = mouse.x; // * pi * 2.; //(1. - mouse.x) * pi * 2.;
    let azimuth = mouse.y; // * pi * 2.;
                           // let pos = vec3(camera_pos.z, 0., 0.);
    let az = azimuth.cos() * 100.;
    let c = vec3(r.cos() * az, r.sin() * az, azimuth.sin() * 100.);

    // println!("camera_pos: {:?}", camera_pos);
    // let quat = Quat::from_axis_angle(vec3(0., 1., 0.), r);
    // let model_mat =
    //     Mat4::from_scale_rotation_translation(vec3(1., 1., 1.), quat, vec3(camera_pos.z, 0., 0.));
    // let model_mat = Mat4::from_translation(vec3(camera_pos.z * 0.785398 * 2., 0., 0.));

    let model_mat = Mat4::look_at_rh(
        //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
        vec3(0., 0., 0.),
        camera_pos,
        // vec3(10., camera_pos.y, camera_pos.x), //+ camera_pos.z
        //vec3(camera_pos.x, camera_pos.z, camera_pos.y),
        //vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
        Vec3::Z,
    );

    let mx_view = Mat4::look_at_rh(
        //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
        camera_pos,
        c.add(camera_pos),
        // vec3(10. + camera_pos.z, camera_pos.y, camera_pos.x), //+ camera_pos.z
        //vec3(camera_pos.x, camera_pos.z, camera_pos.y),
        //vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
        Vec3::Z,
    );

    // let mx_view = Mat4::from_rotation_z(r) * Mat4::from_rotation_y(azimuth);

    // let mx_view = Mat4::look_at_rh(
    //     //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
    //     vec3(camera_pos.x, camera_pos.z, camera_pos.y),
    //     vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
    //     Vec3::Z,
    // );

    (mx_view, mx_projection, model_mat)
}

pub fn render_loop(core: &mut Core, iteration: u64) -> Result<(), wgpu::SurfaceError> {
    // frame!("Render");
    let output = core.surface.get_current_texture()?;

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // TODO is this expensive? only sometimes?
    core.gui
        .render(&core.queue, core.global.get("value2".to_string()));

    let instance_buffers = core.ent_manager.render_ents(iteration, &core.device);

    // frame!("ent build::end");

    core.global.smooth_cam_pos = core.global.smooth_cam_pos * 0.1 + core.global.cam_pos * 0.9;
    let cam_pos = if core.global.debug {
        core.global.smooth_cam_pos + core.global.debug_camera_pos
    } else {
        core.global.smooth_cam_pos
    };

    // let v = core.global.get_mut("value".to_string());
    // *v += 0.002;
    // if *v > 1. {
    //     *v = 0.
    // }
    // core.global.smooth_cam_rot =
    //     core.global.smooth_cam_rot * 0.1 + core.global.simple_cam_rot * 0.9;
    core.global.smooth_cam_rot = core.global.simple_cam_rot;

    let (mx_view, mx_persp, _mx_model) = generate_matrix(
        core.size.width as f32 / core.size.height as f32,
        // *v * 2. * std::f32::consts::PI,
        cam_pos,
        core.global.smooth_cam_rot,
    );

    crate::ray::trace(core, mx_persp, mx_view);

    let mx_view_ref: &[f32; 16] = mx_view.as_ref();
    let mx_persp_ref: &[f32; 16] = mx_persp.as_ref();

    let time_ref: [f32; 16] = [
        core.global.iteration as f32 / 30.,
        core.size.width as f32,
        core.size.height as f32,
        core.global.screen_effects.crt_resolution,
        core.global.screen_effects.corner_harshness,
        core.global.screen_effects.corner_ease,
        core.global.screen_effects.glitchiness,
        core.global.screen_effects.lumen_threshold,
        core.global.screen_effects.dark_factor,
        core.global.screen_effects.low_range,
        core.global.screen_effects.high_range,
        core.global.screen_effects.modernize,
        0.,
        0.,
        0.,
        0.,
    ];

    let size1 = bytemuck::cast_slice(mx_view_ref);
    let size2 = bytemuck::cast_slice(mx_persp_ref);
    let size3 = bytemuck::cast_slice(&time_ref);

    core.queue.write_buffer(&core.uniform_buf, 0, size1);
    core.queue.write_buffer(&core.uniform_buf, 64, size2);
    core.queue.write_buffer(&core.uniform_buf, 128, size3);

    let mut encoder = core
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    encoder.push_debug_group("World Render");
    {
        let switch = core.switch_board.read();
        let bg = switch.background;
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &core.post.post_texture_view, //&core.post.post_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: bg.x as f64,
                        g: bg.y as f64,
                        b: bg.z as f64,
                        a: bg.w as f64,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &core.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        //skybox space
        {
            render_pass.set_pipeline(&core.gui.gui_pipeline);
            render_pass.set_bind_group(0, &core.gui.sky_group, &[]);
            render_pass.draw(0..4, 0..4);
        }

        //world space
        {
            render_pass.set_pipeline(&core.render_pipeline);
            render_pass.set_bind_group(0, &core.main_bind_group, &[]);
            render_pass.set_bind_group(1, &core.entity_bind_group, &[]);
            let chunks = core.world.get_chunk_models();

            for c in chunks {
                if c.buffers.is_some() {
                    let b = c.buffers.as_ref().unwrap();
                    render_pass.set_index_buffer(b.1.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.set_vertex_buffer(0, b.0.slice(..));
                    render_pass.set_vertex_buffer(1, c.instance_buffer.slice(..));
                    render_pass.draw_indexed(0..c.ind_data.len() as u32, 0, 0..1);
                }
            }

            for (model, instance_buffer, size) in instance_buffers.iter() {
                render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);

                render_pass.draw_indexed(0..model.index_count as u32, 0, 0..*size as _);
            }

            if core.ent_manager.specks.len() > 0 {
                // let m = &core.model_manager.PLANE;

                // render_pass.set_vertex_buffer(0, m.vertex_buf.slice(..));
                // render_pass.set_vertex_buffer(1, speck_buffer.slice(..));
                // render_pass.set_index_buffer(m.index_buf.slice(..), m.index_format);
                // render_pass.draw_indexed(0..m.index_count as u32, 0, 0..speck_instances.len() as _);
            }
        }

        //gui space
        {
            render_pass.set_pipeline(&core.gui.gui_pipeline);
            render_pass.set_bind_group(0, &core.gui.gui_group, &[]);
            render_pass.draw(0..4, 0..4);

            // frame!("render pass");
            //render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);
            //render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
            //render_pass.draw_indexed(0..model.index_count as u32, 0, 0..1);
        }
    }
    // drop(render_pass);
    encoder.pop_debug_group();
    // frame!("pop_debug_group");

    // TODO screen grab
    // let texture_extent = wgpu::Extent3d {
    //     width: core.config.width as u32,
    //     height: core.config.height as u32,
    //     depth_or_array_layers: 1,
    // };

    // encoder.copy_texture_to_texture(
    //     output.texture.as_image_copy(),
    //     core.post.post_texture.as_image_copy(),
    //     texture_extent,
    // );

    encoder.push_debug_group("Post Render");
    {
        let mut post_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Post Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, //&core.post.post_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        {
            post_pass.set_pipeline(&core.post.post_pipeline);
            post_pass.set_bind_group(0, &core.post.post_bind_group, &[]);
            post_pass.draw(0..4, 0..4);
            // frame!("post pass");
        }
    }

    core.queue.submit(iter::once(encoder.finish()));
    // frame!("encoder.finish()");
    output.present();

    // frame!("END RENDER");

    Ok(())
}

pub fn log(str: String) {
    crate::log::log(format!("🖌render::{}", str));
}
