use memory_rs::process::process_wrapper::Process;
use winapi::um::winuser;
use winapi::um::winuser::{GetCursorPos, SetCursorPos, GetAsyncKeyState};
use winapi::shared::windef::{POINT};
use std::io::{Error, ErrorKind};
use std::thread;
use std::time::{Duration, Instant};
use std::f32;

const INITIAL_POS: i32 = 500;

#[naked]
unsafe fn shellcode() {
    llvm_asm!("
    push r11
    lea r11,[rip+0x200-0x9];
    pushf
    push rax
    mov eax, [r11-0x10]
    test eax, eax
    pop rax
    je not_zero
    movaps xmm4,[r11+0x40]
    movaps xmm5,[r11]
    movaps xmm6,[r11+0x20] // 0x220
    push rbx
    mov rbx,[r11+0x60]
    mov [rax+0xAC],rbx
    pop rbx


not_zero:
    movaps [r11],xmm5
    movaps [r11+0x20],xmm6
    movaps [r11+0x40],xmm4 // camera rotation
    
    // load fov
    push rbx
    mov rbx,[rax+0xAC]
    mov [r11+0x60],rbx
    pop rbx

    popf
    pop r11
    movaps [rsp+0x48],xmm4 // adjusted offset of stack pointer + 8
    ret
    nop;nop;nop;nop;
    ": : : : "volatile", "intel");
}

fn calc_new_focus_point(cam_x: f32, cam_z: f32,
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


pub fn main() -> Result<(), Error> {
    let mut mouse_pos: POINT = POINT::default();

    // latest mouse positions
    let mut latest_x = 0;
    let mut latest_y = 0;

    println!("
    INSTRUCTIONS:

    PAUSE - Activate/Deactivate Free Camera
    END - Pause the cinematic
    DEL - Deattach Mouse

    UP, DOWN, LEFT, RIGHT - Move in the direction you're pointing
    PG UP, PG DOWN - Increase/Decrease speed multiplier
    F1, F2 - Increase/Decrease FOV respectively

    WARNING: Once you deattach the camera (PAUSE), your mouse will be set in a fixed
    position, so in order to attach/deattach the mouse to the camera, you can
    press DEL

    WARNING: If you're in freeroam and you stop hearing audio, it's probably
    because you have the paused option activated, simply press END to deactivate it.

    ");

    println!("Waiting for the game to start");
    let yakuza = loop {
        match Process::new("Yakuza0.exe") {
            Ok(p) => break p,
            Err(_) => (),
        }

        thread::sleep(Duration::from_secs(5));
    };
    println!("Game hooked");

    let entry_point: usize = 0x18FD38;

    // function that changes the focal length of the cinematics, when
    // active, nop this
    let focal_length_f: Vec<u8> = vec![0xF3, 0x0F, 0x11, 0x89, 0xAC, 0x00, 0x00, 0x00];
    let focal_length_offset = 0x187616;

    // WIP: Pause the cinematics of the world.
    let pause_cinematic_f: Vec<u8> = vec![0x41, 0x8A, 0x8E, 0xC9, 0x00, 0x00, 0x00];
    let pause_cinematic_rep: Vec<u8> = vec![0xB1, 0x01, 0x90, 0x90, 0x90, 0x90, 0x90];
    let pause_cinematic_offset = 0xB720DE;
    let mut pause_world = false;

    let p_shellcode = yakuza.inject_shellcode(entry_point, 5,
        shellcode as usize as *const u8);


    let mut active = false;
    let mut capture_mouse = false;

    let mut restart_mouse = false;

    let mut speed_scale = 1.;

    let mut fov = 0.83;


    // avoid fov jump
    yakuza.write_value::<f32>(p_shellcode + 0x260, fov);
    loop {
        if capture_mouse & restart_mouse {
            unsafe { SetCursorPos(INITIAL_POS, INITIAL_POS) };
            restart_mouse = !restart_mouse;
            latest_x = INITIAL_POS;
            latest_y = INITIAL_POS;
            continue;
        }

        let start = Instant::now();

        // poll rate
        thread::sleep(Duration::from_millis(10));
        unsafe { GetCursorPos(&mut mouse_pos) };
        let duration = start.elapsed().as_millis() as f32;

        let speed_x = ((mouse_pos.x - latest_x) as f32)/duration/5./(40.-fov*10.);
        let speed_y = ((mouse_pos.y - latest_y) as f32)/duration/5./(40.-fov*10.);

        // focus position
        let mut f_cam_x = yakuza.read_value::<f32>(p_shellcode + 0x200);
        let mut f_cam_y = yakuza.read_value::<f32>(p_shellcode + 0x204);
        let mut f_cam_z = yakuza.read_value::<f32>(p_shellcode + 0x208);

        // camera position
        let mut p_cam_x = yakuza.read_value::<f32>(p_shellcode + 0x220);
        let mut p_cam_y = yakuza.read_value::<f32>(p_shellcode + 0x224);
        let mut p_cam_z = yakuza.read_value::<f32>(p_shellcode + 0x228);

        // relative camera position
        let r_cam_x = f_cam_x - p_cam_x;
        let r_cam_y = f_cam_y - p_cam_y;
        let r_cam_z = f_cam_z - p_cam_z;

        let mut dp_forward = 0.;
        let mut dp_sides = 0.;

        unsafe {
            if (GetAsyncKeyState(winuser::VK_UP) as u32 & 0x8000) != 0 {
                dp_forward = 0.1*speed_scale;
            }
            if (GetAsyncKeyState(winuser::VK_DOWN) as u32 & 0x8000) != 0 {
                dp_forward = -0.1*speed_scale;
            }

            if (GetAsyncKeyState(winuser::VK_LEFT) as u32 & 0x8000) != 0 {
                dp_sides = 0.1*speed_scale;
            }
            if (GetAsyncKeyState(winuser::VK_RIGHT) as u32 & 0x8000) != 0 {
                dp_sides = -0.1*speed_scale;
            }

            if (GetAsyncKeyState(winuser::VK_F1) as u32 & 0x8000) != 0 {
                fov += if fov < 3.13 { 0.01 } else { 0. };
            }
            if (GetAsyncKeyState(winuser::VK_F2) as u32 & 0x8000) != 0 {
                fov -= if fov > 0.1 { 0.01 } else { 0. };
            }
        }

        let (r_cam_x, r_cam_z, r_cam_y) = calc_new_focus_point(r_cam_x,
            r_cam_z, r_cam_y, speed_x, speed_y);

        f_cam_x = p_cam_x + r_cam_x + dp_forward*r_cam_x + dp_sides*r_cam_z;
        f_cam_z = p_cam_z + r_cam_z + dp_forward*r_cam_z - dp_sides*r_cam_x;
        f_cam_y = p_cam_y + r_cam_y + dp_forward*r_cam_y;

        p_cam_x = p_cam_x + dp_forward*r_cam_x + dp_sides*r_cam_z;
        p_cam_z = p_cam_z + dp_forward*r_cam_z - dp_sides*r_cam_x;
        p_cam_y = p_cam_y + dp_forward*r_cam_y;

        if capture_mouse {
            yakuza.write_value::<f32>(p_shellcode + 0x200, f_cam_x);
            yakuza.write_value::<f32>(p_shellcode + 0x204, f_cam_y);
            yakuza.write_value::<f32>(p_shellcode + 0x208, f_cam_z);

            yakuza.write_value::<f32>(p_shellcode + 0x220, p_cam_x);
            yakuza.write_value::<f32>(p_shellcode + 0x224, p_cam_y);
            yakuza.write_value::<f32>(p_shellcode + 0x228, p_cam_z);

            yakuza.write_value::<f32>(p_shellcode + 0x240, 0.);
            yakuza.write_value::<f32>(p_shellcode + 0x244, 1.);
            yakuza.write_value::<f32>(p_shellcode + 0x248, 0.);

            yakuza.write_value::<f32>(p_shellcode + 0x260, fov);
        }

        latest_x = mouse_pos.x;
        latest_y = mouse_pos.y;

        // to scroll infinitely
        restart_mouse = !restart_mouse;
        unsafe {
            if (GetAsyncKeyState(winuser::VK_PAUSE) as u32 & 0x8000) != 0 {
                active = !active;
                capture_mouse = active;
                yakuza.write_value::<u32>(p_shellcode + 0x1F0, active as u32);

                let c_status = if active { "Deattached" } else { "Attached" };
                println!("status of camera: {}", c_status);

                if active {
                    yakuza.write_nops(focal_length_offset, focal_length_f.len());
                } else {
                    yakuza.write_aob(focal_length_offset, &focal_length_f);
                }
                thread::sleep(Duration::from_millis(500));
            }

            if active & (GetAsyncKeyState(winuser::VK_DELETE) as u32 & 0x8000 != 0) {
                capture_mouse = !capture_mouse;
                let c_status = if !capture_mouse { "Deattached" } else { "Attached" };
                println!("status of mouse: {}", c_status);
                thread::sleep(Duration::from_millis(500));
            }

            if (GetAsyncKeyState(winuser::VK_PRIOR) as u32 & 0x8000) != 0 {
                speed_scale *= 2.;
                println!("Speed increased, {:.2}", speed_scale);
                thread::sleep(Duration::from_millis(100));
            }

            if (GetAsyncKeyState(winuser::VK_NEXT) as u32 & 0x8000) != 0 {
                if speed_scale > 1e-5 {
                    speed_scale /= 2.;
                    println!("Speed decreased, {:.2}", speed_scale);
                } else {
                    println!("Cannot be decreased, {:.2}", speed_scale);
                }
                thread::sleep(Duration::from_millis(100));
            }

            if (GetAsyncKeyState(winuser::VK_END) as u32 & 0x8000) != 0 {
                pause_world = !pause_world;
                println!("status of pausing: {}", pause_world);
                if pause_world {
                    yakuza.write_aob(pause_cinematic_offset, &pause_cinematic_rep);
                } else {
                    yakuza.write_aob(pause_cinematic_offset, &pause_cinematic_f);
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}
