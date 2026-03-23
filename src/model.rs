/// Hardware model being emulated
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Model {
    /// Original Game Boy (DMG)
    DMG,
    /// Game Boy Color (CGB)
    CGB,
}

impl Model {
    pub fn is_cgb(self) -> bool { self == Model::CGB }
    pub fn is_dmg(self) -> bool { self == Model::DMG }
}
