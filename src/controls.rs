use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

use crate::{globals, State};

pub fn controls_evaluate(event: &WindowEvent, state: &mut State, control_flow: &mut ControlFlow) {
    match event {
        WindowEvent::CloseRequested
        | WindowEvent::KeyboardInput {
            input:
                KeyboardInput {
                    state: ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    ..
                },
            ..
        } => *control_flow = ControlFlow::Exit,
        // WindowEvent::KeyboardInput {
        //     input:
        //         KeyboardInput {
        //             state: ElementState::Pressed,
        //             virtual_keycode: Some(VirtualKeyCode::Space),
        //             ..
        //         },
        //     ..
        // } => {
        //     state.switch_board.write().space = true;
        //     //globals.write().space = true;
        // }
        // WindowEvent::KeyboardInput {
        //     input:
        //         KeyboardInput {
        //             state: ElementState::Released,
        //             virtual_keycode: Some(VirtualKeyCode::Space),
        //             ..
        //         },
        //     ..
        // } => {
        //     state.switch_board.write().space = false;
        //     let input_path = std::path::Path::new("scripts").join("walker.lua");

        //     state
        //         .gui
        //         .add_text(std::fs::read_to_string(input_path).unwrap_or_default());
        //     //globals.write().space = false;
        // }
        WindowEvent::Resized(physical_size) => {
            state.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            // new_inner_size is &mut so w have to dereference it twice
            state.resize(**new_inner_size);
        }
        _ => {
            state.switch_board.write().space = state.input_helper.key_held(VirtualKeyCode::Space);

            if state.input_helper.mouse_pressed(0) {
                match state.input_helper.mouse() {
                    Some((x, y)) => {
                        state.mouse = (x / state.size.width as f32, y / state.size.height as f32);
                        state.value2 = 2.;
                    }
                    _ => {}
                }
            }

            if state.input_helper.key_released(VirtualKeyCode::Grave) {
                state.gui.toggle_output()
            } else {
                let t = state.input_helper.text();

                if t.len() > 0 {
                    let mut neg = 0;
                    let emp: char;
                    let mut st = vec![];
                    for tt in t.iter() {
                        match tt {
                            winit_input_helper::TextChar::Back => {
                                neg += 1;
                            }
                            winit_input_helper::TextChar::Char(c) => st.push(*c),
                        }
                    }
                    crate::log::log(String::from_iter(st));

                    //println!(" {}", String::from_iter(st));
                }
            }
        }
    }

    // match event {
    //     WindowEvent::KeyboardInput {
    //         input:
    //             KeyboardInput {
    //                 state: ElementState::Pressed,
    //                 virtual_keycode: Some(key_out),
    //             },
    //         ..
    //     } => println!("{}", key_out),
    //     _ => {}
    // }
}
