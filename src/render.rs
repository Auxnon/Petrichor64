use crate::Arc;
use glam::{vec3, Mat4, Vec2, Vec3};
use once_cell::sync::OnceCell;
use std::{iter, ops::Add};
// use tracy::frame;

use crate::{
    ent::{Ent, EntityUniforms},
    model::Model,
    Core,
};

/** create rotation matrix from camera position and simple rotation */
pub fn generate_matrix(
    aspect_ratio: f32,
    _rot: f32,
    mut camera_pos: Vec3,
    mouse: Vec2,
) -> (Mat4, Mat4, Mat4) {
    let pi = std::f32::consts::PI;
    let mx_projection = Mat4::perspective_rh(0.785398, aspect_ratio, 1., 6400.0);

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

    // output.texture.
    // output.texture.
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // TODO is this expensive? only sometimes?
    core.gui
        .render(&core.queue, core.global.get("value2".to_string()));
    // frame!("rendered gui texture");

    let mutex = crate::ent_master.read();

    let entity_manager = mutex.get().unwrap();
    let ents = &entity_manager.ent_table;
    // let (entity_manager,ents)=match crate::ent_master.try_read(){
    //     Some(guar)=>{
    //         let entity_manager = mutex.get().unwrap();
    //         entity_manager,
    //     }
    // }

    // MARK PRE
    // frame!("ent build::start");
    let lua_ent_array = ents
        .iter()
        .filter_map(|a| match a.lock() {
            Ok(g) => Some(g.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut ent_array: Vec<(Ent, Arc<OnceCell<Model>>, EntityUniforms)> = vec![];
    for entity in &mut lua_ent_array.iter() {
        match entity_manager.get_from_id(entity.get_id()) {
            Some(o) => {
                let model = o.model.clone();
                let data = o.get_uniform(&entity.clone(), iteration);
                let e = o.clone();
                ent_array.push((e, model, data));
            }
            _ => {}
        }
    }

    drop(ents);
    drop(entity_manager);

    // frame!("ent build::end");

    let v = core.global.get_mut("value".to_string());
    *v += 0.002;
    if *v > 1. {
        *v = 0.
    }

    let (mx_view, mx_persp, _mx_model) = generate_matrix(
        core.size.width as f32 / core.size.height as f32,
        *v * 2. * std::f32::consts::PI,
        core.global.camera_pos,
        core.global.simple_cam_rot,
    );

    crate::ray::trace(core, mx_persp, mx_view);

    let mx_view_ref: &[f32; 16] = mx_view.as_ref();
    let mx_persp_ref: &[f32; 16] = mx_persp.as_ref();

    let time_ref: [f32; 12] = [
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
    ];

    // let iTime=adj[0];
    // let dark_factor:f32=adj[8]; //0.4
    // let low_range:f32=adj[9]; //.05
    // let high_range:f32=adj[10]; //0.6
    // let resolution=vec2<f32>(adj[1],adj[2]);
    // let corner_harshness: f32 =adj[4]; // 1.2
    // let corner_ease: f32 = adj[5]; // 4.0
    // let res: f32 =adj[3]; //  320.0
    // let glitchy: f32 =adj[6]; // 3.0
    // let lumen_threshold:f32=adj[7]; //0.2

    let size1 = bytemuck::cast_slice(mx_view_ref);
    let size2 = bytemuck::cast_slice(mx_persp_ref);
    let size3 = bytemuck::cast_slice(&time_ref);

    core.queue.write_buffer(&core.uniform_buf, 0, size1);
    core.queue.write_buffer(&core.uniform_buf, 64, size2);

    // TODO should use offset of mat4 buffer size, 64 by deffault, is it always?
    core.queue.write_buffer(&core.uniform_buf, 128, size3);

    let m: Mat4 = Mat4::IDENTITY;
    let data = EntityUniforms {
        model: m.to_cols_array_2d(),
        color: [1., 1., 1., 1.],
        uv_mod: [0., 0., 1., 1.],
        effects: [0, 0, 0, 0],
    };

    core.queue.write_buffer(
        &core.entity_uniform_buf,
        0 as wgpu::BufferAddress,
        bytemuck::bytes_of(&data),
    );

    // MARK 1
    // frame!("ent use1::start");
    let neu = true;
    if neu {
        for (entity, _, data) in &mut ent_array.iter() {
            core.queue.write_buffer(
                &core.entity_uniform_buf,
                entity.uniform_offset as wgpu::BufferAddress,
                bytemuck::bytes_of(data),
            );
        }
    }

    // let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
    // let instance_data = ent_array
    //     .iter()
    //     .map(|(entity, _, data)| bytemuck::bytes_of(data))
    //     .collect::<Vec<_>>();
    if !neu {
        let mut buf: Vec<u8> = vec![];
        for (_entity, _, data) in &mut ent_array.iter() {
            // buf.extend_from_slice(bytemuck::bytes_of(data));

            // let int = entity.uniform_offset as usize;

            // let d = bytemuck::bytes_of(data);
            // let v: Vec<u8> = bytemuck::bytes_of(data).to_vec();
            // if v.len() > 0 {
            buf.append(&mut bytemuck::bytes_of(data).to_vec());
            //int..int + v.len(),
            // }
            // core.queue.write_buffer(
            //     &core.entity_uniform_buf,
            //     entity.uniform_offset as wgpu::BufferAddress,
            //     bytemuck::bytes_of(data),
            // );
        }
        core.queue.write_buffer(&core.entity_uniform_buf, 0, &buf);
    }

    // frame!("ent use1::end");

    // RED

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
            color_attachments: &[wgpu::RenderPassColorAttachment {
                //MARK view
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
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &core.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        //world space
        {
            render_pass.set_pipeline(&core.render_pipeline);
            render_pass.set_bind_group(0, &core.bind_group, &[]);

            // let c = core.world.get_chunk_mut(0, 0, 0);

            // let go = match core.switch_board.try_read() {
            //     Some(sw) => !sw.space,
            //     _ => true,
            // };

            let chunks = core.world.get_chunk_models(&core.device);
            // println!("------chunks {} ------", chunks.len());
            render_pass.set_bind_group(1, &core.entity_bind_group, &[0]);
            for c in chunks {
                if c.buffers.is_some() {
                    // println!("chunk {} pos: {} ind: {}", c.key, c.pos, c.ind_data.len());
                    let b = c.buffers.as_ref().unwrap();
                    render_pass.set_index_buffer(b.1.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.set_vertex_buffer(0, b.0.slice(..));
                    render_pass.draw_indexed(0..c.ind_data.len() as u32, 0, 0..1);
                }
            }

            // {
            //     let c = core.world.get_chunk_mut(0, 0, -2);
            //     // println!(
            //     //     "\nchunk drawing {:?} \n and {:?}",
            //     //     c.ind_data,
            //     //     c.vert_data.iter().map(|u| u.to_string()).join(", ")
            //     // );
            //     // println!("chunk {}", c.pos);
            //     let b = c.buffers.as_ref().unwrap();
            //     render_pass.set_bind_group(1, &core.entity_bind_group, &[0]);
            //     render_pass.set_index_buffer(b.1.slice(..), wgpu::IndexFormat::Uint32);
            //     render_pass.set_vertex_buffer(0, b.0.slice(..));
            //     render_pass.draw_indexed(0..c.ind_data.len() as u32, 0, 0..1);
            // }

            // MARK 2
            // frame!("entity use2::start");

            for (entity, model, _) in &mut ent_array.iter() {
                let m = model.get().unwrap();
                render_pass.set_bind_group(1, &core.entity_bind_group, &[entity.uniform_offset]);
                render_pass.set_index_buffer(m.index_buf.slice(..), m.index_format);
                render_pass.set_vertex_buffer(0, m.vertex_buf.slice(..));
                render_pass.draw_indexed(0..m.index_count as u32, 0, 0..1);
            }
            // frame!("ent use2::end");
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
        // BLUE
        //post process

        //render_pass.draw(0..3, 0..1);
        //render_pass.draw_indexed(0..core.index_count as u32, 0, 0..1);
    }
    // drop(render_pass);
    encoder.pop_debug_group();
    // frame!("pop_debug_group");

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
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &view, //&core.post.post_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
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
