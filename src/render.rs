use std::{
    f32::consts::PI,
    iter,
    ops::{Div, DivAssign, Mul},
};

use glam::{vec3, vec4, Mat3, Mat4, Quat, Vec3, Vec4Swizzles};

use crate::{
    ent::{self, Ent, EntityUniforms},
    lua_define, State,
};

pub fn generate_matrix(aspect_ratio: f32, rot: f32, camera_pos: Vec3) -> (Mat4, Mat4) {
    let mx_projection = Mat4::perspective_rh(0.785398, aspect_ratio, 1., 800.0);

    let r = 0.5f32;
    let mx_view = Mat4::look_at_rh(
        vec3(92. * r.cos(), -128., 82.0),
        vec3(0f32, 0.0, 0.0),
        Vec3::Z,
    );

    let mx_view = Mat4::IDENTITY;

    let mx_view = Mat4::look_at_rh(camera_pos, vec3(0., -10., 0.) + camera_pos, Vec3::Z);

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

    let (mx_view, mx_persp) = generate_matrix(
        state.size.width as f32 / state.size.height as f32,
        state.value * 2. * std::f32::consts::PI,
        state.camera_pos,
    );

    if true {
        let inv = (mx_persp * mx_view).inverse();
        //let viewport = vec4(0., 0., state.size.width as f32, state.size.height as f32);
        //let screen = vec3(state.mouse.0, state.mouse.1, 20.);
        // let mut pp = vec3(screen.x - viewport.x, viewport.w - screen.y - 1., screen.z);
        // pp.y -= viewport.y;
        // let out = vec3(
        //     (2. * pp.x) / viewport.z - 1.,
        //     (2. * pp.y) / viewport.w - 1.,
        //     2. * pp.z - 1.,
        // );

        //         float pt_x = (point.x / screenSize.x) * 2.f - 1.f;
        // float pt_y = -(point.y / screenSize.y) * 2.f + 1.f;

        //                       z value from 0.f to 1.f for d3d
        // vec4 origin = math::mul(vec4(pt_x, pt_y, -1.f, 1.f), VPinv);
        // origin.w = 1.0f / origin.w;
        // origin.x *= origin.w;
        // origin.y *= origin.w;
        // origin.z *= origin.w;

        // let screen = vec4(state.mouse.0 * 2. - 1., -state.mouse.1 * 2. + 1., -1., 1.);
        // let mut origin = inv.mul_vec4(screen);
        // origin.w = 1. / origin.w;
        // origin.x *= origin.w;
        // origin.y *= origin.w;
        // origin.z *= origin.w;

        // let cam_eye = vec4(
        //     92. * (state.value * 2. * std::f32::consts::PI).cos(),
        //     -128.,
        //     82.0,
        //     1.,
        // );

        let cam_eye = vec4(
            state.camera_pos.x,
            state.camera_pos.y,
            state.camera_pos.z,
            1.,
        );

        let cam_center = vec4(0., 0., 0., 1.);

        let win_coord = vec3(state.mouse_active_pos.0, state.mouse_active_pos.1, 1.);

        let screen_coord = vec4(
            2. * (win_coord.x) - 1.,
            -2. * (win_coord.y) + 1.,
            win_coord.z, //2. * win.z - 1.,
            1.,
        );
        let near_coord = vec4(
            2. * (win_coord.x) - 1.,
            -2. * (win_coord.y) + 1.,
            0.05, //2. * win.z - 1.,
            1.,
        );
        let mut screen_unproj = inv.mul_vec4(screen_coord);
        screen_unproj.div_assign(screen_unproj.w);

        let mut near_unproj = inv.mul_vec4(near_coord);
        near_unproj.div_assign(near_unproj.w);

        let dir = (screen_unproj.xyz() - near_unproj.xyz()).normalize(); // - (state.camera_pos + vec3(0., -10., 0.))

        let out_point = dir.mul(20.); //dir.xyz().normalize().mul(20.);

        //screen_unproj.normalize().mul(10.);
        //result.div_assign(40.);

        /*


            let _2: N = na::convert(2.0);
        let transform = (proj * model).try_inverse().unwrap_or_else(TMat4::zeros);
        let pt = TVec4::new(
            _2 * (win.x - viewport.x) / viewport.z - N::one(),
            _2 * (win.y - viewport.y) / viewport.w - N::one(),
            _2 * win.z - N::one(),
            N::one(),
        );

        let result = transform * pt;
        result.fixed_rows::<U3>(0) / result.w

        */

        /*


                 var vector = new THREE.Vector3();
                vector.set(( Control.screenX() / window.innerWidth ) * 2 - 1, - ( Control.screenY() / window.innerHeight ) * 2 + 1,0.05 );
                vector.unproject(camera)
                var dir = vector.sub( camera.position ).normalize();
                var distance = - camera.position.z / dir.z;
                var pos = camera.position.clone().add( dir.multiplyScalar( distance ) );

                pointer.position.x =pos.x;
                pointer.position.y =pos.y
                Control.setVector(pointer.position);

        multiply proj inverse by camera pos matrix?
        unproject(camera) {
                    return this.applyMatrix4(camera.projectionMatrixInverse).applyMatrix4(camera.matrixWorld);
                }
                */
        //transform(out, out, invProjectionView);
        // let v = vec4(
        //     (state.mouse.0 * 2. - 1.),
        //     (state.mouse.1 * 2. - 1.),
        //     10.,
        //     1.,
        // );
        //state.size.width as f32 *
        //state.size.height as f32 *
        //let p = inv.mul_vec4(v);
        // let p = v * inv;

        // let t = inv.mul_vec4(v);
        // let t = (inv.transform_vector3(screen) - center).normalize();
        // let distance = -center.z / t.z;

        // let pos = center + (t.mul(distance));

        if state.value2 >= 1. {
            let typeOf = state.entities.len() % 2 == 0;
            state.value2 = 0.;

            println!(
                "win {}  dir {} world space {}",
                screen_coord, dir, out_point
            );

            state.entities.push(Ent::new(
                out_point,
                0.,
                if typeOf { 1. } else { 1. },
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
                None, //Some("walker".to_string()),
            ))
        }

        state.entities[0].pos = out_point;
    }

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

    let m: Mat4 = Mat4::IDENTITY;
    let data = EntityUniforms {
        model: m.to_cols_array_2d(),
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

        let rotation = Mat4::from_rotation_z(entity.rotation);

        let quat = Quat::from_axis_angle(vec3(0., 0., 1.), entity.rotation);

        // let transform = cgmath::Decomposed {
        //     disp: entity.pos.mul(16.),
        //     rot: ),
        //     //rot: cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation)),
        //     scale: entity.scale * 16.,
        // };
        let s = entity.scale;

        entity.matrix =
            Mat4::from_scale_rotation_translation(vec3(s, s, s), quat, entity.pos.mul(16.));

        // DEV i32
        /*
                let rotation = cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation));

                let v = entity.pos.mul(16.).cast::<i32>().unwrap();
                let rot = cgmath::Quaternion::<i32>::from_sv(
                    entity.rotation as i32,
                    cgmath::Vector3::<i32>::new(0, 0, 1),
                );
                let transform = cgmath::Decomposed::<cgmath::Vector3<i32>, cgmath::Quaternion<i32>> {
                    disp: v,
                    rot: rot,
                    //rot: cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation)),
        <<<<<<< Updated upstream
                    scale: (entity.scale * 16.) as i32,
        =======
                    scale: entity.scale,
        >>>>>>> Stashed changes
                };
                let matrix = cgmath::Matrix4::<i32>::from(transform);
                */

        let data = EntityUniforms {
            model: entity.matrix.to_cols_array_2d(),
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
