#[derive(Clone, Copy, Debug, Default)]
pub struct Kinetic;

impl Kinetic {
    pub fn of(_item: &str) -> Option<Self> {
        None
    }
}

pub fn first_person_use(
    _kinetic: &Kinetic,
    _ticks_since_hit: f32,
    _time_held: f32,
    _left: bool,
) -> glam::Mat4 {
    glam::Mat4::IDENTITY
}

pub fn third_person_arm(
    _kinetic: &Kinetic,
    _time_held: f32,
    _right: bool,
    _hand_offset: (f32, f32),
    _fall_or_swim: bool,
) -> (f32, f32, f32) {
    (0.0, 0.0, 0.0)
}

pub fn third_person_use_item(
    _kinetic: &Kinetic,
    _time_held: f32,
    _swing: f32,
    _x: f32,
    _left: bool,
) -> f32 {
    1.0
}