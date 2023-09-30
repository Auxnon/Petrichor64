#![windows_subsystem = "windows"]
use bytemuck::{Pod, Zeroable};

use crate::{model::ModelManager, types::ValueMap};
use rustc_hash::FxHashMap;

use std::{
    mem,
    rc::Rc,
    sync::mpsc::{channel, Receiver, Sender},
};
// use tracy::frame;
use crate::{
    bundle::{self, BundleManager},
    command,
    ent_manager::EntManager,
    global::{Global, GuiStyle},
    log::Loggy,
    lua_define::MainPacket,
    world::{self, World},
};
use crate::{gui::Gui, log::LogType};
use glam::{vec2, vec3, Mat4};

pub struct Core {
    pub global: Global,
    pub world: World,
    pub pitcher: Sender<MainPacket>,
    pub catcher: Receiver<MainPacket>,
    pub loop_helper: spin_sleep::LoopHelper,
    pub ent_manager: EntManager,
    pub model_manager: ModelManager,
    pub bundle_manager: BundleManager,
    pub loggy: Loggy,
    pub gui: Gui,
    pub cli_thread_receiver: Receiver<String>,
}
impl Core {
    pub async fn new(pitcher: Sender<MainPacket>) -> Core {
        let mut ent_manager = EntManager::new();
        let global = Global::new();
        let mut loggy = Loggy::new();
        let world = World::new(loggy.make_sender());
        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(60.0); // limit to X FPS if possible
        let model_manager = ModelManager::init();
        let mut gui = Gui::new((256, 256), &mut loggy);

        let (cli_thread_sender, cli_thread_receiver) = channel::<String>();
        std::thread::spawn(move || loop {
            let mut line = String::new();
            if let Ok(_) = std::io::stdin().read_line(&mut line) {
                cli_thread_sender.send(line).unwrap();
                // if line.trim() == "exit" {
                //     std::process::exit(0);
                // }
            }
        });

        Self {
            global,
            world,
            loop_helper,
            ent_manager,
            model_manager,
            pitcher,
            bundle_manager: BundleManager::new(),
            loggy,
            gui,
            cli_thread_receiver,
        }
    }
}
