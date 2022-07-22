pub struct Pad {
    pub south: f32,
    pub north: f32,
    pub east: f32,
    pub west: f32,
    pub dleft: f32,
    pub dright: f32,
    pub dup: f32,
    pub ddown: f32,

    pub rtrigger: f32,
    pub ltrigger: f32,
    pub rbumper: f32,
    pub lbumper: f32,

    pub start: f32,
    pub select: f32,
    pub laxisx: f32,
    pub laxisy: f32,
    pub raxisx: f32,
    pub raxisy: f32,
    pub lstick: f32,
    pub rstick: f32,
}

impl Pad {
    pub fn new() -> Pad {
        Pad {
            south: 0.0,
            north: 0.0,
            east: 0.0,
            west: 0.0,
            dleft: 0.0,
            dright: 0.0,
            dup: 0.0,
            ddown: 0.0,

            rtrigger: 0.0,
            ltrigger: 0.0,
            rbumper: 0.0,
            lbumper: 0.0,

            start: 0.0,
            select: 0.0,
            laxisx: 0.0,
            laxisy: 0.0,
            raxisx: 0.0,
            raxisy: 0.0,
            lstick: 0.0,
            rstick: 0.0,
        }
    }
    pub fn check(&self, str: String) -> f32 {
        match str.as_str() {
            "south" => self.south,
            "north" => self.north,
            "east" => self.east,
            "west" => self.west,
            "dleft" => self.dleft,
            "dright" => self.dright,
            "dup" => self.dup,
            "ddown" => self.ddown,
            "laxisx" => self.laxisx,
            "laxisy" => self.laxisy,
            "raxisx" => self.raxisx,
            "raxisy" => self.raxisy,
            "lstick" => self.lstick,
            "rstick" => self.rstick,
            "rtrigger" => self.rtrigger,
            "ltrigger" => self.ltrigger,
            "rbumper" => self.rbumper,
            "lbumper" => self.lbumper,
            "start" => self.start,
            "select" => self.select,

            _ => 0.0,
        }
    }
}
