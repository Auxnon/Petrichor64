use std::{
    f32::consts::PI,
    iter,
    ops::{Div, DivAssign, Mul},
    slice::SliceIndex,
};

use glam::{vec3, vec4, Mat3, Mat4, Quat, Vec2, Vec3, Vec4Swizzles};

use crate::{
    ent::{self, Ent, EntityUniforms},
    Core,
};

pub fn generate_matrix(
    aspect_ratio: f32,
    rot: f32,
    camera_pos: Vec3,
    mouse: Vec2,
) -> (Mat4, Mat4, Mat4) {
    let pi = std::f32::consts::PI;
    let mx_projection = Mat4::perspective_rh(0.785398, aspect_ratio, 1., 6400.0);

    let r = 0.5f32;
    let mx_view = Mat4::look_at_rh(
        vec3(92. * r.cos(), -128., 82.0),
        vec3(0f32, 0.0, 0.0),
        Vec3::Z,
    );

    let mx_view = Mat4::IDENTITY;
    // let r = pi * (0.5 + (mouse.0 % 100.) / 50.);
    // let azimuth = pi * (0.5 + (mouse.1 % 100.) / 50.);
    let r = (1. - mouse.x) * pi * 2.;
    let azimuth = mouse.y * pi * 2.;
    let pos = vec3(camera_pos.z, 0., 0.);
    let az = azimuth.cos() * 100.;
    let c = vec3(r.cos() * az, r.sin() * az, azimuth.sin() * 100.);

    // let quat = Quat::from_axis_angle(vec3(0., 1., 0.), r);
    // let model_mat =
    //     Mat4::from_scale_rotation_translation(vec3(1., 1., 1.), quat, vec3(camera_pos.z, 0., 0.));
    // let model_mat = Mat4::from_translation(vec3(camera_pos.z * 0.785398 * 2., 0., 0.));

    let model_mat = Mat4::look_at_rh(
        //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
        vec3(0., 0., 0.),
        vec3(10., camera_pos.y, camera_pos.x), //+ camera_pos.z
        //vec3(camera_pos.x, camera_pos.z, camera_pos.y),
        //vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
        Vec3::Z,
    );

    let mx_view = Mat4::look_at_rh(
        //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
        pos,
        c,
        // vec3(10. + camera_pos.z, camera_pos.y, camera_pos.x), //+ camera_pos.z
        //vec3(camera_pos.x, camera_pos.z, camera_pos.y),
        //vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
        Vec3::Z,
    );

    //let mx_view = Mat4::from_rotation_z(r) * Mat4::from_rotation_y(azimuth);

    // let mx_view = Mat4::look_at_rh(
    //     //vec3(r.cos() * 128., r.sin() * 128., camera_pos.y),
    //     vec3(camera_pos.x, camera_pos.z, camera_pos.y),
    //     vec3(camera_pos.x, camera_pos.z - 16., camera_pos.y),
    //     Vec3::Z,
    // );

    (mx_view, mx_projection, model_mat)
}

pub fn render_loop(core: &mut Core) -> Result<(), wgpu::SurfaceError> {
    let output = core.surface.get_current_texture()?;
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    core.gui.render(
        &core.device,
        &core.queue,
        core.global.get("value2".to_string()),
    );

    let mut mutex = crate::ent_master.lock();

    let mut ent_tabler = crate::ent_table.lock();
    println!(
        "we have this many ents {} first is {}",
        ent_tabler.len(),
        if ent_tabler.len() > 0 {
            ent_tabler[0].x
        } else {
            -1.
        }
    );

    //log(format!("hooked {}", path));
    let entity_manager = mutex.get_mut().unwrap();
    let ents = &mut entity_manager.entities;

    // let mut v = core.global.get("value".to_string());
    let v = core.global.get_mut("value".to_string());
    *v += 0.002;
    if *v > 1. {
        *v = 0.
    }

    let (mx_view, mx_persp, mx_model) = generate_matrix(
        core.size.width as f32 / core.size.height as f32,
        *v * 2. * std::f32::consts::PI,
        core.global.camera_pos,
        core.global.mouse_active_pos,
    );

    if true {
        //let trans = Mat4::from_translation(core.camera_pos);
        //let mm = Mat4::from_translation(core.camera_pos) * Mat4::IDENTITY;
        //mx_model
        let tran = Mat4::from_translation(core.global.camera_pos);
        // let inv = (mx_persp * mx_view).inverse(); //mx_persp * mx_model * mx_view
        //let inv = mx_persp.inverse() * mx_view.inverse(); //mx_persp * mx_model * mx_view
        //let inv = (mx_persp.inverse() * Mat4::IDENTITY) * mx_view.inverse();

        let inv = (mx_persp * mx_view).inverse();
        //let viewport = vec4(0., 0., core.size.width as f32, core.size.height as f32);
        //let screen = vec3(core.mouse.0, core.mouse.1, 20.);
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

        // let screen = vec4(core.mouse.0 * 2. - 1., -core.mouse.1 * 2. + 1., -1., 1.);
        // let mut origin = inv.mul_vec4(screen);
        // origin.w = 1. / origin.w;
        // origin.x *= origin.w;
        // origin.y *= origin.w;
        // origin.z *= origin.w;

        // let cam_eye = vec4(
        //     92. * (core.value * 2. * std::f32::consts::PI).cos(),
        //     -128.,
        //     82.0,
        //     1.,
        // );
        // let cam_proj = (mx_persp).project_point3(core.camera_pos);
        // println!("cam pos proj {}", cam_proj);

        let cam_eye = vec4(
            core.global.camera_pos.x,
            core.global.camera_pos.y,
            core.global.camera_pos.z,
            1.,
        );

        let aspect = core.size.width as f32 / core.size.height as f32;
        // let persp2 = nalgebra::Perspective3::new(aspect, 0.785398, 1., 1600.);

        //let cam_center = vec4(0., 0., 0., 1.);
        let cam_center = vec3(core.global.camera_pos.z, 0., 0.); // vec4(core.camera_pos.x, 0., core.camera_pos.y, 1.);

        let win_coord = vec3(
            core.global.mouse_active_pos.x,
            core.global.mouse_active_pos.y,
            0.,
        );

        let near_coord = vec4(
            2. * (win_coord.x) - 1.,
            -2. * (win_coord.y) + 1.,
            win_coord.z, //2. * win.z - 1.,
            1.,
        );
        let far_coord = vec4(
            2. * (win_coord.x) - 1.,
            -2. * (win_coord.y) + 1.,
            1., //2. * win.z - 1.,
            1.,
        );

        //let n2 = persp2.unproject_point(&nalgebra::Point3::new(n.x, n.y, n.z));
        //let f2 = persp2.unproject_point(&nalgebra::Point3::new(f.x, f.y, f.z));
        // let dir2 = n2 - f2;
        //persp2.unproject_point(p)
        let FoV = 1.27323980963;

        //far_coord.x *= FoV * FoV;
        //far_coord.y *= FoV * FoV;
        //inv.project_point3(other)
        let near_unproj = inv.project_point3(near_coord.xyz());

        // println!("{}", near_unproj);

        let far_unproj = inv.project_point3(far_coord.xyz());
        //let cam_unproj = inv.project_point3(core.camera_pos);

        let dir = (near_unproj - far_unproj).normalize();

        // - (core.camera_pos + vec3(0., -10., 0.))
        //dir = mx_view.inverse().project_point3(dir);
        //     var d = Vector3.Dot(planeP, -planeN);
        // var t = -(d + rayP.z * planeN.z + rayP.y * planeN.y + rayP.x * planeN.x) / (rayD.z * planeN.z + rayD.y * planeN.y + rayD.x * planeN.x);
        // return rayP + t * rayD;

        let out_point;

        let PLANE_COLLIDE = true;

        // Calculate distance of intersection point from r.origin.
        // let denominator = planeP.dot(dir); // Vector3.Dot(p.Normal, ray.Direction);
        // let numerator = planeN.dot(near_unproj) + 4.; //+ planeN//Vector3.Dot(p.Normal, ray.Position) + p.D;
        // let t = -(numerator / denominator);

        // Calculate the picked position on the y = 0 plane.
        // out_point = near_unproj + dir * t;
        // println!("near_unproj {}", near_unproj);
        if PLANE_COLLIDE {
            // let planeP = vec3(16., 0., 0.) - near_unproj;
            let planeP = vec3(0., 0., -6.);
            let planeN = vec3(0., 0., 1.);

            let rayP = far_unproj; // + vec3(10., 0., 0.);
            let rayD = dir;
            let d = planeP.dot(-planeN);
            let t = -(d + rayP.z * planeN.z + rayP.y * planeN.y + rayP.x * planeN.x)
                / (rayD.z * planeN.z + rayD.y * planeN.y + rayD.x * planeN.x);

            out_point = rayP + t * rayD;

            // screen_unproj; //.normalize().mul(20.); //dir.xyz().normalize().mul(20.);
        } else {
            out_point = dir.mul(-16.); // + cam_center.xyz();
        }
        core.global.cursor_projected_pos = out_point;

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
        //     (core.mouse.0 * 2. - 1.),
        //     (core.mouse.1 * 2. - 1.),
        //     10.,
        //     1.,
        // );
        //core.size.width as f32 *
        //core.size.height as f32 *
        //let p = inv.mul_vec4(v);
        // let p = v * inv;

        // let t = inv.mul_vec4(v);
        // let t = (inv.transform_vector3(screen) - center).normalize();
        // let distance = -center.z / t.z;

        // let pos = center + (t.mul(distance));
        // let ent_guard=ent_master.lock();
        // ent_guard.get_mut(slice);

        if core.global.get("value2".to_string()) >= 1. {
            let typeOf = ents.len() % 2 == 0;
            core.global.set("value2".to_string(), 0.);

            println!("  dir {} world space {}", dir, out_point);

            ents.push(Ent::new(
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
                (ents.len() as u64 * core.uniform_alignment) as u32,
                typeOf,
                None, //Some("walker".to_string()),
            ))
        }

        ents[0].pos = out_point;
    }

    // let rot = cgmath::Matrix4::from_angle_y(a);
    // //let mx_ref: = mx_total.as_ref();
    // let mx_totals = rot * core.camera_matrix;
    let mx_view_ref: &[f32; 16] = mx_view.as_ref();
    let mx_persp_ref: &[f32; 16] = mx_persp.as_ref();

    let time_ref: [f32; 4] = ([
        core.global.get("value".to_string()),
        0.,
        core.size.width as f32,
        core.size.height as f32,
    ]);

    core.queue
        .write_buffer(&core.uniform_buf, 0, bytemuck::cast_slice(mx_view_ref));
    core.queue
        .write_buffer(&core.uniform_buf, 64, bytemuck::cast_slice(mx_persp_ref));

    //TODO should use offset of mat4 buffer size, 64 by deffault, is it always?
    core.queue
        .write_buffer(&core.uniform_buf, 128, bytemuck::cast_slice(&time_ref));

    let m: Mat4 = Mat4::IDENTITY;
    let data = EntityUniforms {
        model: m.to_cols_array_2d(),
        color: [1., 1., 1., 1.],
        uv_mod: [0., 0., 1., 1.],
        effects: [0, 0, 0, 0],
    };
    //println!("model {} pos {} {}", i, entity.tex.x, entity.tex.y);
    core.queue.write_buffer(
        &core.entity_uniform_buf,
        0 as wgpu::BufferAddress,
        bytemuck::bytes_of(&data),
    );

    for (i, entity) in &mut ents.iter_mut().enumerate() {
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
        core.queue.write_buffer(
            &core.entity_uniform_buf,
            entity.uniform_offset as wgpu::BufferAddress,
            bytemuck::bytes_of(&data),
        );
    }

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
                view: &view,
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
            let c = core.world.get_chunk_mut(0, 0, 0);
            if c.buffers.is_some() {
                let b = c.buffers.as_ref().unwrap();
                render_pass.set_bind_group(1, &core.entity_bind_group, &[0]);
                render_pass.set_index_buffer(b.1.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.set_vertex_buffer(0, b.0.slice(..));
                render_pass.draw_indexed(0..c.ind_data.len() as u32, 0, 0..1);
            }

            for entity in ents {
                let model = entity.model.get().unwrap();
                render_pass.set_bind_group(1, &core.entity_bind_group, &[entity.uniform_offset]);
                render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);
                render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
                render_pass.draw_indexed(0..model.index_count as u32, 0, 0..1);
            }
        }

        //gui space
        {
            // let res = core.gui.model.get();
            // if res.is_some() {
            //     let model = res.unwrap();
            render_pass.set_pipeline(&core.gui.gui_pipeline);
            render_pass.set_bind_group(0, &core.gui.gui_group, &[]);
            render_pass.draw(0..4, 0..4);
            //render_pass.set_index_buffer(model.index_buf.slice(..), model.index_format);
            //render_pass.set_vertex_buffer(0, model.vertex_buf.slice(..));
            //render_pass.draw_indexed(0..model.index_count as u32, 0, 0..1);
            //}
        }

        //render_pass.draw(0..3, 0..1);
        //render_pass.draw_indexed(0..core.index_count as u32, 0, 0..1);
    }
    encoder.pop_debug_group();

    // queue.write_buffer(
    //     &self.entity_uniform_buf,
    //     entity.uniform_offset as wgpu::BufferAddress,
    //     bytemuck::bytes_of(&data),
    // );

    core.queue.submit(iter::once(encoder.finish()));
    output.present();

    entity_manager.check_create();

    Ok(())
}

pub fn log(str: String) {
    crate::log::log(format!("🖌render::{}", str));
}
