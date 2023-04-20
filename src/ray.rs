use std::ops::Mul;

use glam::{vec3, vec4, Mat4, Vec4Swizzles};

use crate::Core;

pub fn trace(core: &mut Core, mx_persp: Mat4, mx_view: Mat4) {
    //let trans = Mat4::from_translation(core.camera_pos);
    //let mm = Mat4::from_translation(core.camera_pos) * Mat4::IDENTITY;
    //mx_model

    let _tran = Mat4::from_translation(core.global.cam_pos);

    // let inv = (mx_persp * mx_view).inverse(); //mx_persp * mx_model * mx_view
    //let inv = mx_persp.inverse() * mx_view.inverse(); //mx_persp * mx_model * mx_view
    //let inv = (mx_persp.inverse() * Mat4::IDENTITY) * mx_view.inverse();

    let inv = mx_view.inverse() * mx_persp.inverse();
    // out.proj_position=globals.proj_mat*(globals.view_mat*world_pos);

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

    let _cam_eye = vec4(
        core.global.cam_pos.x,
        core.global.cam_pos.y,
        core.global.cam_pos.z,
        1.,
    );

    let _aspect = core.gfx.size.width as f32 / core.gfx.size.height as f32;
    // let persp2 = nalgebra::Perspective3::new(aspect, 0.785398, 1., 1600.);

    //let cam_center = vec4(0., 0., 0., 1.);
    let cam_center = vec3(
        core.global.cam_pos.x,
        core.global.cam_pos.x,
        core.global.cam_pos.z,
    );

    let win_coord = vec3(core.global.mouse_pos.x, core.global.mouse_pos.y, 0.);

    // let win_coord = vec3(0., 0., 0.);

    // if (EngineStore.LastCreatedEngine?.isNDCHalfZRange) {
    //     screenSource.z = sourceZ;
    // } else {
    //     screenSource.z = 2 * sourceZ - 1.0;
    // }

    // Vector3.TransformCoordinatesToRef(source, matrix, result);
    //     const m = matrix.m;
    //     const num = source._x * m[3] + source._y * m[7] + source._z * m[11] + m[15];
    //     if (Scalar.WithinEpsilon(num, 1.0)) {
    //         result.scaleInPlace(1.0 / num);
    //     }
    //     return result;

    // public static TransformCoordinatesFromFloatsToRef<T extends Vector4>(x: number, y: number, z: number, transformation: DeepImmutable<Matrix>, result: T): T {
    //     const m = transformation.m;
    //     const rx = x * m[0] + y * m[4] + z * m[8] + m[12];
    //     const ry = x * m[1] + y * m[5] + z * m[9] + m[13];
    //     const rz = x * m[2] + y * m[6] + z * m[10] + m[14];
    //     const rw = x * m[3] + y * m[7] + z * m[11] + m[15];

    //     result.x = rx;
    //     result.y = ry;
    //     result.z = rz;
    //     result.w = rw;
    //     return result;
    // }

    let near_coord = vec3(
        2. * (win_coord.x) - 1.,
        -2. * (win_coord.y) + 1.,
        0., // win_coord.z, //2. * win.z - 1.,
            // 1.,
    );
    let far_coord = vec3(
        2. * (win_coord.x) - 1.,
        -2. * (win_coord.y) + 1.,
        -1., //2. * win.z - 1.,
             // 1.,
    );

    //let n2 = persp2.unproject_point(&nalgebra::Point3::new(n.x, n.y, n.z));
    //let f2 = persp2.unproject_point(&nalgebra::Point3::new(f.x, f.y, f.z));
    // let dir2 = n2 - f2;
    //persp2.unproject_point(p)
    let _fov = 1.27323980963;

    //far_coord.x *= FoV * FoV;
    //far_coord.y *= FoV * FoV;
    //inv.project_point3(other)
    // inv.
    let near_unproj = inv.project_point3(near_coord);

    // println!("{}", near_unproj);

    let far_unproj = inv.project_point3(far_coord);
    //let cam_unproj = inv.project_point3(core.camera_pos);

    let dir = (near_unproj - far_unproj).normalize();

    let pv = vec3(0., 0., 0.);
    let pn = vec3(0., 0., 1.);
    let denominator = pn.dot(dir);
    let numerator = pn.dot(near_unproj) + 4.;
    let t = -numerator / denominator;
    // let out_point = dir; //near_unproj + t * dir;
    let out_point = dir;

    // let vv = nalgebra_glm::TVec3::new(0., 0., 0.);

    /*let tpersp = nalgebra_glm::TMat4::new(
        mx_persp.x_axis.x,
        mx_persp.x_axis.y,
        mx_persp.x_axis.z,
        mx_persp.x_axis.w,
        mx_persp.y_axis.x,
        mx_persp.y_axis.y,
        mx_persp.y_axis.z,
        mx_persp.y_axis.w,
        mx_persp.z_axis.x,
        mx_persp.z_axis.y,
        mx_persp.z_axis.z,
        mx_persp.z_axis.w,
        mx_persp.w_axis.x,
        mx_persp.w_axis.y,
        mx_persp.w_axis.z,
        mx_persp.w_axis.w,
    );
    let tview = nalgebra_glm::TMat4::new(
        mx_view.x_axis.x,
        mx_view.x_axis.y,
        mx_view.x_axis.z,
        mx_view.x_axis.w,
        mx_view.y_axis.x,
        mx_view.y_axis.y,
        mx_view.y_axis.z,
        mx_view.y_axis.w,
        mx_view.z_axis.x,
        mx_view.z_axis.y,
        mx_view.z_axis.z,
        mx_view.z_axis.w,
        mx_view.w_axis.x,
        mx_view.w_axis.y,
        mx_view.w_axis.z,
        mx_view.w_axis.w,
    );

    let m4 = nalgebra_glm::TVec3::new(core.global.mouse_pos.x, core.global.mouse_pos.y, 0.);
    let t4 = nalgebra_glm::TVec4::new(0., 0., core.size.width as f32, core.size.height as f32);
    let outv: nalgebra_glm::Vec3 = nalgebra_glm::unproject(&m4, &tview, &tpersp, t4);
    let out2 = vec3(outv.x, outv.y, outv.z);*/

    // let out2=vec3;

    // - (core.camera_pos + vec3(0., -10., 0.))
    //dir = mx_view.inverse().project_point3(dir);
    //     var d = Vector3.Dot(planeP, -planeN);
    // var t = -(d + rayP.z * planeN.z + rayP.y * planeN.y + rayP.x * planeN.x) / (rayD.z * planeN.z + rayD.y * planeN.y + rayD.x * planeN.x);
    // return rayP + t * rayD;

    // let out_point;

    let plane_collide = false;

    // Calculate distance of intersection point from r.origin.
    // let denominator = planeP.dot(dir); // Vector3.Dot(p.Normal, ray.Direction);
    // let numerator = planeN.dot(near_unproj) + 4.; //+ planeN//Vector3.Dot(p.Normal, ray.Position) + p.D;
    // let t = -(numerator / denominator);

    // Calculate the picked position on the y = 0 plane.
    // out_point = near_unproj + dir * t;
    // println!("near_unproj {}", near_unproj);
    // if plane_collide {
    //     // let planeP = vec3(16., 0., 0.) - near_unproj;
    //     let plane_p = vec3(0., 0., -6.);
    //     let plane_n = vec3(0., 0., 1.);

    //     let ray_p = far_unproj; // + vec3(10., 0., 0.);
    //     let ray_d = dir;
    //     let d = plane_p.dot(-plane_n);
    //     let t = -(d + ray_p.z * plane_n.z + ray_p.y * plane_n.y + ray_p.x * plane_n.x)
    //         / (ray_d.z * plane_n.z + ray_d.y * plane_n.y + ray_d.x * plane_n.x);

    //     out_point = ray_p + t * ray_d;

    //     // screen_unproj; //.normalize().mul(20.); //dir.xyz().normalize().mul(20.);
    // } else {
    //     out_point = dir.mul(-16.); // + cam_center.xyz();
    // }
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

    // if core.global.get("value2".to_string()) >= 1. {
    //     let _type_of = 0; // DEV ents.len() % 2 == 0;
    //     core.global.set("value2".to_string(), 0.);

    //     println!("  dir {} world space {}", dir, out_point);
    // DEV TODO
    // ents.push(Ent::new(
    //     out_point,
    //     0.,
    //     if typeOf { 1. } else { 1. },
    //     0.,
    //     if typeOf {
    //         "chicken".to_string()
    //     } else {
    //         "package".to_string()
    //     },
    //     if typeOf {
    //         "plane".to_string()
    //     } else {
    //         "package".to_string()
    //     },
    //     (ents.len() as u64 * core.uniform_alignment) as u32,
    //     typeOf,
    //     None, //Some("walker".to_string()),
    // ))
    // }

    // ents[0].pos = out_point; // DEV

    // let rot = cgmath::Matrix4::from_angle_y(a);
    // //let mx_ref: = mx_total.as_ref();
    // let mx_totals = rot * core.camera_matrix;
}
