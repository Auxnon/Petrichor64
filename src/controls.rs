use winit::event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};
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
        WindowEvent::KeyboardInput {
            input:
                KeyboardInput {
                    state: ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::Space),
                    ..
                },
            ..
        } => {
            state.switch_board.write().space = true;
            //globals.write().space = true;
        }
        WindowEvent::KeyboardInput {
            input:
                KeyboardInput {
                    state: ElementState::Released,
                    virtual_keycode: Some(VirtualKeyCode::Space),
                    ..
                },
            ..
        } => {
            state.switch_board.write().space = false;
            let input_path = std::path::Path::new("scripts").join("walker.lua");

            state
                .gui
                .add_text(std::fs::read_to_string(input_path).unwrap_or_default());
            //globals.write().space = false;
        }
        WindowEvent::Resized(physical_size) => {
            state.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            // new_inner_size is &mut so w have to dereference it twice
            state.resize(**new_inner_size);
        }
        _ => {}
    }
}
