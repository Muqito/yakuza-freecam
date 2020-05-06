pub struct Camera {
    p_camera_x: f32,
    p_camera_y: f32,
    p_camera_z: f32,

    f_camera_x: f32,
    f_camera_y: f32,
    f_camera_z: f32,

    active: bool,

    capture_mouse: bool,
    restart_mouse: bool,

    speed_scale: f32,

    focal_lenght_f: Vec<u8>,

    pause_cinematics_f: Vec<u8>,
    pause_cinematics: bool,

    base_address: u64
}

pub fn calc_new_focus_point(cam_x: f32, cam_z: f32,
    cam_y: f32, speed_x: f32, speed_y: f32) -> (f32, f32, f32) {

    // use spherical coordinates to add speed
    let theta = cam_z.atan2(cam_x) + speed_x;

    let phi = (cam_x.powi(2) + cam_z.powi(2)).sqrt().atan2(cam_y) +
        speed_y;

    let r = (cam_x.powi(2) + cam_y.powi(2) + cam_z.powi(2)).sqrt();

    let r_cam_x = r*theta.cos()*phi.sin();
    let r_cam_z = r*theta.sin()*phi.sin();
    let r_cam_y = r*phi.cos();

    (r_cam_x, r_cam_z, r_cam_y)
}

