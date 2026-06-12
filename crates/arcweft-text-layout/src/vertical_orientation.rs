// This file is generated from Unicode VerticalOrientation.txt.
// Source: https://www.unicode.org/Public/UCD/latest/ucd/VerticalOrientation.txt
// Unicode data version: 17.0.0
// Do not edit range data by hand.

/// Unicode data version used by the generated vertical orientation table.
pub const UNICODE_VERTICAL_ORIENTATION_VERSION: &str = "17.0.0";

/// Unicode `Vertical_Orientation` property values from UAX #50.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnicodeVerticalOrientation {
    /// Upright, same orientation as in the code charts.
    Upright,
    /// Rotated 90 degrees clockwise compared to the code charts.
    Rotated,
    /// Transformed typographically, with fallback to upright.
    TransformedUpright,
    /// Transformed typographically, with fallback to rotated.
    TransformedRotated,
}

/// Returns the UAX #50 `Vertical_Orientation` property for one scalar value.
pub fn unicode_vertical_orientation(ch: char) -> UnicodeVerticalOrientation {
    let codepoint = ch as u32;
    let mut low = 0usize;
    let mut high = VERTICAL_ORIENTATION_RANGES.len();
    while low < high {
        let mid = low + (high - low) / 2;
        let range = VERTICAL_ORIENTATION_RANGES[mid];
        if codepoint < range.start {
            high = mid;
        } else if codepoint > range.end {
            low = mid + 1;
        } else {
            return range.orientation;
        }
    }
    UnicodeVerticalOrientation::Rotated
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VerticalOrientationRange {
    start: u32,
    end: u32,
    orientation: UnicodeVerticalOrientation,
}

const VERTICAL_ORIENTATION_RANGES: &[VerticalOrientationRange] = &[
    VerticalOrientationRange {
        start: 0x0000,
        end: 0x001F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0020,
        end: 0x0020,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0021,
        end: 0x0023,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0024,
        end: 0x0024,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0025,
        end: 0x0027,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0028,
        end: 0x0028,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0029,
        end: 0x0029,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x002A,
        end: 0x002A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x002B,
        end: 0x002B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x002C,
        end: 0x002C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x002D,
        end: 0x002D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x002E,
        end: 0x002F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0030,
        end: 0x0039,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x003A,
        end: 0x003B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x003C,
        end: 0x003E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x003F,
        end: 0x0040,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0041,
        end: 0x005A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x005B,
        end: 0x005B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x005C,
        end: 0x005C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x005D,
        end: 0x005D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x005E,
        end: 0x005E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x005F,
        end: 0x005F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0060,
        end: 0x0060,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0061,
        end: 0x007A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x007B,
        end: 0x007B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x007C,
        end: 0x007C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x007D,
        end: 0x007D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x007E,
        end: 0x007E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x007F,
        end: 0x007F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0080,
        end: 0x009F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A0,
        end: 0x00A0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A1,
        end: 0x00A1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A2,
        end: 0x00A5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A6,
        end: 0x00A6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A7,
        end: 0x00A7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00A8,
        end: 0x00A8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00A9,
        end: 0x00A9,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00AA,
        end: 0x00AA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00AB,
        end: 0x00AB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00AC,
        end: 0x00AC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00AD,
        end: 0x00AD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00AE,
        end: 0x00AE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00AF,
        end: 0x00AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B0,
        end: 0x00B0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B1,
        end: 0x00B1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00B2,
        end: 0x00B3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B4,
        end: 0x00B4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B5,
        end: 0x00B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B6,
        end: 0x00B7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B8,
        end: 0x00B8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00B9,
        end: 0x00B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00BA,
        end: 0x00BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00BB,
        end: 0x00BB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00BC,
        end: 0x00BE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00BF,
        end: 0x00BF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00C0,
        end: 0x00D6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00D7,
        end: 0x00D7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00D8,
        end: 0x00F6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x00F7,
        end: 0x00F7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x00F8,
        end: 0x00FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0100,
        end: 0x017F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0180,
        end: 0x01BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x01BB,
        end: 0x01BB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x01BC,
        end: 0x01BF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x01C0,
        end: 0x01C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x01C4,
        end: 0x024F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0250,
        end: 0x0293,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0294,
        end: 0x0295,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0296,
        end: 0x02AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02B0,
        end: 0x02C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02C2,
        end: 0x02C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02C6,
        end: 0x02D1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02D2,
        end: 0x02DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02E0,
        end: 0x02E4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02E5,
        end: 0x02E9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02EA,
        end: 0x02EB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x02EC,
        end: 0x02EC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02ED,
        end: 0x02ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02EE,
        end: 0x02EE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x02EF,
        end: 0x02FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0300,
        end: 0x036F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0370,
        end: 0x0373,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0374,
        end: 0x0374,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0375,
        end: 0x0375,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0376,
        end: 0x0377,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x037A,
        end: 0x037A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x037B,
        end: 0x037D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x037E,
        end: 0x037E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x037F,
        end: 0x037F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0384,
        end: 0x0385,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0386,
        end: 0x0386,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0387,
        end: 0x0387,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0388,
        end: 0x038A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x038C,
        end: 0x038C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x038E,
        end: 0x03A1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x03A3,
        end: 0x03F5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x03F6,
        end: 0x03F6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x03F7,
        end: 0x03FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0400,
        end: 0x0481,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0482,
        end: 0x0482,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0483,
        end: 0x0487,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0488,
        end: 0x0489,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x048A,
        end: 0x04FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0500,
        end: 0x052F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0531,
        end: 0x0556,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0559,
        end: 0x0559,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x055A,
        end: 0x055F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0560,
        end: 0x0588,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0589,
        end: 0x0589,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x058A,
        end: 0x058A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x058D,
        end: 0x058E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x058F,
        end: 0x058F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0591,
        end: 0x05BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05BE,
        end: 0x05BE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05BF,
        end: 0x05BF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C0,
        end: 0x05C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C1,
        end: 0x05C2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C3,
        end: 0x05C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C4,
        end: 0x05C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C6,
        end: 0x05C6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05C7,
        end: 0x05C7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05D0,
        end: 0x05EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05EF,
        end: 0x05F2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x05F3,
        end: 0x05F4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0600,
        end: 0x0605,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0606,
        end: 0x0608,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0609,
        end: 0x060A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x060B,
        end: 0x060B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x060C,
        end: 0x060D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x060E,
        end: 0x060F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0610,
        end: 0x061A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x061B,
        end: 0x061B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x061C,
        end: 0x061C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x061D,
        end: 0x061F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0620,
        end: 0x063F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0640,
        end: 0x0640,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0641,
        end: 0x064A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x064B,
        end: 0x065F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0660,
        end: 0x0669,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x066A,
        end: 0x066D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x066E,
        end: 0x066F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0670,
        end: 0x0670,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0671,
        end: 0x06D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06D4,
        end: 0x06D4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06D5,
        end: 0x06D5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06D6,
        end: 0x06DC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06DD,
        end: 0x06DD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06DE,
        end: 0x06DE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06DF,
        end: 0x06E4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06E5,
        end: 0x06E6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06E7,
        end: 0x06E8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06E9,
        end: 0x06E9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06EA,
        end: 0x06ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06EE,
        end: 0x06EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06F0,
        end: 0x06F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06FA,
        end: 0x06FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06FD,
        end: 0x06FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x06FF,
        end: 0x06FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0700,
        end: 0x070D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x070F,
        end: 0x070F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0710,
        end: 0x0710,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0711,
        end: 0x0711,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0712,
        end: 0x072F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0730,
        end: 0x074A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x074D,
        end: 0x074F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0750,
        end: 0x077F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0780,
        end: 0x07A5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07A6,
        end: 0x07B0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07B1,
        end: 0x07B1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07C0,
        end: 0x07C9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07CA,
        end: 0x07EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07EB,
        end: 0x07F3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07F4,
        end: 0x07F5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07F6,
        end: 0x07F6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07F7,
        end: 0x07F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07FA,
        end: 0x07FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07FD,
        end: 0x07FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x07FE,
        end: 0x07FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0800,
        end: 0x0815,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0816,
        end: 0x0819,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x081A,
        end: 0x081A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x081B,
        end: 0x0823,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0824,
        end: 0x0824,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0825,
        end: 0x0827,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0828,
        end: 0x0828,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0829,
        end: 0x082D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0830,
        end: 0x083E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0840,
        end: 0x0858,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0859,
        end: 0x085B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x085E,
        end: 0x085E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0860,
        end: 0x086A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0870,
        end: 0x0887,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0888,
        end: 0x0888,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0889,
        end: 0x088F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0890,
        end: 0x0891,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0897,
        end: 0x089F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x08A0,
        end: 0x08C8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x08C9,
        end: 0x08C9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x08CA,
        end: 0x08E1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x08E2,
        end: 0x08E2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x08E3,
        end: 0x08FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0900,
        end: 0x0902,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0903,
        end: 0x0903,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0904,
        end: 0x0939,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x093A,
        end: 0x093A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x093B,
        end: 0x093B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x093C,
        end: 0x093C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x093D,
        end: 0x093D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x093E,
        end: 0x0940,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0941,
        end: 0x0948,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0949,
        end: 0x094C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x094D,
        end: 0x094D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x094E,
        end: 0x094F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0950,
        end: 0x0950,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0951,
        end: 0x0957,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0958,
        end: 0x0961,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0962,
        end: 0x0963,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0964,
        end: 0x0965,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0966,
        end: 0x096F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0970,
        end: 0x0970,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0971,
        end: 0x0971,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0972,
        end: 0x097F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0980,
        end: 0x0980,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0981,
        end: 0x0981,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0982,
        end: 0x0983,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0985,
        end: 0x098C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x098F,
        end: 0x0990,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0993,
        end: 0x09A8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09AA,
        end: 0x09B0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09B2,
        end: 0x09B2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09B6,
        end: 0x09B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09BC,
        end: 0x09BC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09BD,
        end: 0x09BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09BE,
        end: 0x09C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09C1,
        end: 0x09C4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09C7,
        end: 0x09C8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09CB,
        end: 0x09CC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09CD,
        end: 0x09CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09CE,
        end: 0x09CE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09D7,
        end: 0x09D7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09DC,
        end: 0x09DD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09DF,
        end: 0x09E1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09E2,
        end: 0x09E3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09E6,
        end: 0x09EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09F0,
        end: 0x09F1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09F2,
        end: 0x09F3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09F4,
        end: 0x09F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09FA,
        end: 0x09FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09FB,
        end: 0x09FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09FC,
        end: 0x09FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09FD,
        end: 0x09FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x09FE,
        end: 0x09FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A01,
        end: 0x0A02,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A03,
        end: 0x0A03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A05,
        end: 0x0A0A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A0F,
        end: 0x0A10,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A13,
        end: 0x0A28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A2A,
        end: 0x0A30,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A32,
        end: 0x0A33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A35,
        end: 0x0A36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A38,
        end: 0x0A39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A3C,
        end: 0x0A3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A3E,
        end: 0x0A40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A41,
        end: 0x0A42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A47,
        end: 0x0A48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A4B,
        end: 0x0A4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A51,
        end: 0x0A51,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A59,
        end: 0x0A5C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A5E,
        end: 0x0A5E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A66,
        end: 0x0A6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A70,
        end: 0x0A71,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A72,
        end: 0x0A74,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A75,
        end: 0x0A75,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A76,
        end: 0x0A76,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A81,
        end: 0x0A82,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A83,
        end: 0x0A83,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A85,
        end: 0x0A8D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A8F,
        end: 0x0A91,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0A93,
        end: 0x0AA8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AAA,
        end: 0x0AB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AB2,
        end: 0x0AB3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AB5,
        end: 0x0AB9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ABC,
        end: 0x0ABC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ABD,
        end: 0x0ABD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ABE,
        end: 0x0AC0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AC1,
        end: 0x0AC5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AC7,
        end: 0x0AC8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AC9,
        end: 0x0AC9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ACB,
        end: 0x0ACC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ACD,
        end: 0x0ACD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AD0,
        end: 0x0AD0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AE0,
        end: 0x0AE1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AE2,
        end: 0x0AE3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AE6,
        end: 0x0AEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AF0,
        end: 0x0AF0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AF1,
        end: 0x0AF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AF9,
        end: 0x0AF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0AFA,
        end: 0x0AFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B01,
        end: 0x0B01,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B02,
        end: 0x0B03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B05,
        end: 0x0B0C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B0F,
        end: 0x0B10,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B13,
        end: 0x0B28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B2A,
        end: 0x0B30,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B32,
        end: 0x0B33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B35,
        end: 0x0B39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B3C,
        end: 0x0B3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B3D,
        end: 0x0B3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B3E,
        end: 0x0B3E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B3F,
        end: 0x0B3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B40,
        end: 0x0B40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B41,
        end: 0x0B44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B47,
        end: 0x0B48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B4B,
        end: 0x0B4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B4D,
        end: 0x0B4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B55,
        end: 0x0B56,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B57,
        end: 0x0B57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B5C,
        end: 0x0B5D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B5F,
        end: 0x0B61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B62,
        end: 0x0B63,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B66,
        end: 0x0B6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B70,
        end: 0x0B70,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B71,
        end: 0x0B71,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B72,
        end: 0x0B77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B82,
        end: 0x0B82,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B83,
        end: 0x0B83,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B85,
        end: 0x0B8A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B8E,
        end: 0x0B90,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B92,
        end: 0x0B95,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B99,
        end: 0x0B9A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B9C,
        end: 0x0B9C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0B9E,
        end: 0x0B9F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BA3,
        end: 0x0BA4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BA8,
        end: 0x0BAA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BAE,
        end: 0x0BB9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BBE,
        end: 0x0BBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BC0,
        end: 0x0BC0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BC1,
        end: 0x0BC2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BC6,
        end: 0x0BC8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BCA,
        end: 0x0BCC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BCD,
        end: 0x0BCD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BD0,
        end: 0x0BD0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BD7,
        end: 0x0BD7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BE6,
        end: 0x0BEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BF0,
        end: 0x0BF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BF3,
        end: 0x0BF8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BF9,
        end: 0x0BF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0BFA,
        end: 0x0BFA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C00,
        end: 0x0C00,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C01,
        end: 0x0C03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C04,
        end: 0x0C04,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C05,
        end: 0x0C0C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C0E,
        end: 0x0C10,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C12,
        end: 0x0C28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C2A,
        end: 0x0C39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C3C,
        end: 0x0C3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C3D,
        end: 0x0C3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C3E,
        end: 0x0C40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C41,
        end: 0x0C44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C46,
        end: 0x0C48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C4A,
        end: 0x0C4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C55,
        end: 0x0C56,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C58,
        end: 0x0C5A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C5C,
        end: 0x0C5D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C60,
        end: 0x0C61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C62,
        end: 0x0C63,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C66,
        end: 0x0C6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C77,
        end: 0x0C77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C78,
        end: 0x0C7E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C7F,
        end: 0x0C7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C80,
        end: 0x0C80,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C81,
        end: 0x0C81,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C82,
        end: 0x0C83,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C84,
        end: 0x0C84,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C85,
        end: 0x0C8C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C8E,
        end: 0x0C90,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0C92,
        end: 0x0CA8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CAA,
        end: 0x0CB3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CB5,
        end: 0x0CB9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CBC,
        end: 0x0CBC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CBD,
        end: 0x0CBD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CBE,
        end: 0x0CBE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CBF,
        end: 0x0CBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CC0,
        end: 0x0CC4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CC6,
        end: 0x0CC6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CC7,
        end: 0x0CC8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CCA,
        end: 0x0CCB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CCC,
        end: 0x0CCD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CD5,
        end: 0x0CD6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CDC,
        end: 0x0CDE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CE0,
        end: 0x0CE1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CE2,
        end: 0x0CE3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CE6,
        end: 0x0CEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CF1,
        end: 0x0CF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0CF3,
        end: 0x0CF3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D00,
        end: 0x0D01,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D02,
        end: 0x0D03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D04,
        end: 0x0D0C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D0E,
        end: 0x0D10,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D12,
        end: 0x0D3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D3B,
        end: 0x0D3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D3D,
        end: 0x0D3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D3E,
        end: 0x0D40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D41,
        end: 0x0D44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D46,
        end: 0x0D48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D4A,
        end: 0x0D4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D4D,
        end: 0x0D4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D4E,
        end: 0x0D4E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D4F,
        end: 0x0D4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D54,
        end: 0x0D56,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D57,
        end: 0x0D57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D58,
        end: 0x0D5E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D5F,
        end: 0x0D61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D62,
        end: 0x0D63,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D66,
        end: 0x0D6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D70,
        end: 0x0D78,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D79,
        end: 0x0D79,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D7A,
        end: 0x0D7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D81,
        end: 0x0D81,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D82,
        end: 0x0D83,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D85,
        end: 0x0D96,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0D9A,
        end: 0x0DB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DB3,
        end: 0x0DBB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DBD,
        end: 0x0DBD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DC0,
        end: 0x0DC6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DCA,
        end: 0x0DCA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DCF,
        end: 0x0DD1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DD2,
        end: 0x0DD4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DD6,
        end: 0x0DD6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DD8,
        end: 0x0DDF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DE6,
        end: 0x0DEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DF2,
        end: 0x0DF3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0DF4,
        end: 0x0DF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E01,
        end: 0x0E30,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E31,
        end: 0x0E31,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E32,
        end: 0x0E33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E34,
        end: 0x0E3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E3F,
        end: 0x0E3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E40,
        end: 0x0E45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E46,
        end: 0x0E46,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E47,
        end: 0x0E4E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E4F,
        end: 0x0E4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E50,
        end: 0x0E59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E5A,
        end: 0x0E5B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E81,
        end: 0x0E82,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E84,
        end: 0x0E84,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E86,
        end: 0x0E8A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0E8C,
        end: 0x0EA3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EA5,
        end: 0x0EA5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EA7,
        end: 0x0EB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EB1,
        end: 0x0EB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EB2,
        end: 0x0EB3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EB4,
        end: 0x0EBC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EBD,
        end: 0x0EBD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EC0,
        end: 0x0EC4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EC6,
        end: 0x0EC6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EC8,
        end: 0x0ECE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0ED0,
        end: 0x0ED9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0EDC,
        end: 0x0EDF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F00,
        end: 0x0F00,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F01,
        end: 0x0F03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F04,
        end: 0x0F12,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F13,
        end: 0x0F13,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F14,
        end: 0x0F14,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F15,
        end: 0x0F17,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F18,
        end: 0x0F19,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F1A,
        end: 0x0F1F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F20,
        end: 0x0F29,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F2A,
        end: 0x0F33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F34,
        end: 0x0F34,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F35,
        end: 0x0F35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F36,
        end: 0x0F36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F37,
        end: 0x0F37,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F38,
        end: 0x0F38,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F39,
        end: 0x0F39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F3A,
        end: 0x0F3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F3B,
        end: 0x0F3B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F3C,
        end: 0x0F3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F3D,
        end: 0x0F3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F3E,
        end: 0x0F3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F40,
        end: 0x0F47,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F49,
        end: 0x0F6C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F71,
        end: 0x0F7E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F7F,
        end: 0x0F7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F80,
        end: 0x0F84,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F85,
        end: 0x0F85,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F86,
        end: 0x0F87,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F88,
        end: 0x0F8C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F8D,
        end: 0x0F97,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0F99,
        end: 0x0FBC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FBE,
        end: 0x0FC5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FC6,
        end: 0x0FC6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FC7,
        end: 0x0FCC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FCE,
        end: 0x0FCF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FD0,
        end: 0x0FD4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FD5,
        end: 0x0FD8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0FD9,
        end: 0x0FDA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1000,
        end: 0x102A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x102B,
        end: 0x102C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x102D,
        end: 0x1030,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1031,
        end: 0x1031,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1032,
        end: 0x1037,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1038,
        end: 0x1038,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1039,
        end: 0x103A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x103B,
        end: 0x103C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x103D,
        end: 0x103E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x103F,
        end: 0x103F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1040,
        end: 0x1049,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x104A,
        end: 0x104F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1050,
        end: 0x1055,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1056,
        end: 0x1057,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1058,
        end: 0x1059,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x105A,
        end: 0x105D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x105E,
        end: 0x1060,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1061,
        end: 0x1061,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1062,
        end: 0x1064,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1065,
        end: 0x1066,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1067,
        end: 0x106D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x106E,
        end: 0x1070,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1071,
        end: 0x1074,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1075,
        end: 0x1081,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1082,
        end: 0x1082,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1083,
        end: 0x1084,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1085,
        end: 0x1086,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1087,
        end: 0x108C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x108D,
        end: 0x108D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x108E,
        end: 0x108E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x108F,
        end: 0x108F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1090,
        end: 0x1099,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x109A,
        end: 0x109C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x109D,
        end: 0x109D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x109E,
        end: 0x109F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10A0,
        end: 0x10C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10C7,
        end: 0x10C7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10CD,
        end: 0x10CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10D0,
        end: 0x10FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10FB,
        end: 0x10FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10FC,
        end: 0x10FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x10FD,
        end: 0x10FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1100,
        end: 0x11FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x1200,
        end: 0x1248,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x124A,
        end: 0x124D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1250,
        end: 0x1256,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1258,
        end: 0x1258,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x125A,
        end: 0x125D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1260,
        end: 0x1288,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x128A,
        end: 0x128D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1290,
        end: 0x12B0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12B2,
        end: 0x12B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12B8,
        end: 0x12BE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12C0,
        end: 0x12C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12C2,
        end: 0x12C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12C8,
        end: 0x12D6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x12D8,
        end: 0x1310,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1312,
        end: 0x1315,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1318,
        end: 0x135A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x135D,
        end: 0x135F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1360,
        end: 0x1368,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1369,
        end: 0x137C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1380,
        end: 0x138F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1390,
        end: 0x1399,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x13A0,
        end: 0x13F5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x13F8,
        end: 0x13FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1400,
        end: 0x1400,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1401,
        end: 0x166C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x166D,
        end: 0x166D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x166E,
        end: 0x166E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x166F,
        end: 0x167F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x1680,
        end: 0x1680,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1681,
        end: 0x169A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x169B,
        end: 0x169B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x169C,
        end: 0x169C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x16A0,
        end: 0x16EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x16EB,
        end: 0x16ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x16EE,
        end: 0x16F0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x16F1,
        end: 0x16F8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1700,
        end: 0x1711,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1712,
        end: 0x1714,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1715,
        end: 0x1715,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x171F,
        end: 0x171F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1720,
        end: 0x1731,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1732,
        end: 0x1733,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1734,
        end: 0x1734,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1735,
        end: 0x1736,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1740,
        end: 0x1751,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1752,
        end: 0x1753,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1760,
        end: 0x176C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x176E,
        end: 0x1770,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1772,
        end: 0x1773,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1780,
        end: 0x17B3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17B4,
        end: 0x17B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17B6,
        end: 0x17B6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17B7,
        end: 0x17BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17BE,
        end: 0x17C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17C6,
        end: 0x17C6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17C7,
        end: 0x17C8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17C9,
        end: 0x17D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17D4,
        end: 0x17D6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17D7,
        end: 0x17D7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17D8,
        end: 0x17DA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17DB,
        end: 0x17DB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17DC,
        end: 0x17DC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17DD,
        end: 0x17DD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17E0,
        end: 0x17E9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x17F0,
        end: 0x17F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1800,
        end: 0x1805,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1806,
        end: 0x1806,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1807,
        end: 0x180A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x180B,
        end: 0x180D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x180E,
        end: 0x180E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x180F,
        end: 0x180F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1810,
        end: 0x1819,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1820,
        end: 0x1842,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1843,
        end: 0x1843,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1844,
        end: 0x1878,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1880,
        end: 0x1884,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1885,
        end: 0x1886,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1887,
        end: 0x18A8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x18A9,
        end: 0x18A9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x18AA,
        end: 0x18AA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x18B0,
        end: 0x18F5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x18F6,
        end: 0x18FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x1900,
        end: 0x191E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1920,
        end: 0x1922,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1923,
        end: 0x1926,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1927,
        end: 0x1928,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1929,
        end: 0x192B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1930,
        end: 0x1931,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1932,
        end: 0x1932,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1933,
        end: 0x1938,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1939,
        end: 0x193B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1940,
        end: 0x1940,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1944,
        end: 0x1945,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1946,
        end: 0x194F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1950,
        end: 0x196D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1970,
        end: 0x1974,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1980,
        end: 0x19AB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x19B0,
        end: 0x19C9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x19D0,
        end: 0x19D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x19DA,
        end: 0x19DA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x19DE,
        end: 0x19DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x19E0,
        end: 0x19FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A00,
        end: 0x1A16,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A17,
        end: 0x1A18,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A19,
        end: 0x1A1A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A1B,
        end: 0x1A1B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A1E,
        end: 0x1A1F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A20,
        end: 0x1A54,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A55,
        end: 0x1A55,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A56,
        end: 0x1A56,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A57,
        end: 0x1A57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A58,
        end: 0x1A5E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A60,
        end: 0x1A60,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A61,
        end: 0x1A61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A62,
        end: 0x1A62,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A63,
        end: 0x1A64,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A65,
        end: 0x1A6C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A6D,
        end: 0x1A72,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A73,
        end: 0x1A7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A7F,
        end: 0x1A7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A80,
        end: 0x1A89,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1A90,
        end: 0x1A99,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1AA0,
        end: 0x1AA6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1AA7,
        end: 0x1AA7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1AA8,
        end: 0x1AAD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1AB0,
        end: 0x1ABD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1ABE,
        end: 0x1ABE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1ABF,
        end: 0x1ADD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1AE0,
        end: 0x1AEB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B00,
        end: 0x1B03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B04,
        end: 0x1B04,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B05,
        end: 0x1B33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B34,
        end: 0x1B34,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B35,
        end: 0x1B35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B36,
        end: 0x1B3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B3B,
        end: 0x1B3B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B3C,
        end: 0x1B3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B3D,
        end: 0x1B41,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B42,
        end: 0x1B42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B43,
        end: 0x1B44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B45,
        end: 0x1B4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B4E,
        end: 0x1B4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B50,
        end: 0x1B59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B5A,
        end: 0x1B60,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B61,
        end: 0x1B6A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B6B,
        end: 0x1B73,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B74,
        end: 0x1B7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B7D,
        end: 0x1B7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B80,
        end: 0x1B81,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B82,
        end: 0x1B82,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1B83,
        end: 0x1BA0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BA1,
        end: 0x1BA1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BA2,
        end: 0x1BA5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BA6,
        end: 0x1BA7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BA8,
        end: 0x1BA9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BAA,
        end: 0x1BAA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BAB,
        end: 0x1BAD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BAE,
        end: 0x1BAF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BB0,
        end: 0x1BB9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BBA,
        end: 0x1BBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BC0,
        end: 0x1BE5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BE6,
        end: 0x1BE6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BE7,
        end: 0x1BE7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BE8,
        end: 0x1BE9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BEA,
        end: 0x1BEC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BED,
        end: 0x1BED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BEE,
        end: 0x1BEE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BEF,
        end: 0x1BF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BF2,
        end: 0x1BF3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1BFC,
        end: 0x1BFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C00,
        end: 0x1C23,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C24,
        end: 0x1C2B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C2C,
        end: 0x1C33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C34,
        end: 0x1C35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C36,
        end: 0x1C37,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C3B,
        end: 0x1C3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C40,
        end: 0x1C49,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C4D,
        end: 0x1C4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C50,
        end: 0x1C59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C5A,
        end: 0x1C77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C78,
        end: 0x1C7D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C7E,
        end: 0x1C7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C80,
        end: 0x1C8A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1C90,
        end: 0x1CBA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CBD,
        end: 0x1CBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CC0,
        end: 0x1CC7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CD0,
        end: 0x1CD2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CD3,
        end: 0x1CD3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CD4,
        end: 0x1CE0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CE1,
        end: 0x1CE1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CE2,
        end: 0x1CE8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CE9,
        end: 0x1CEC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CED,
        end: 0x1CED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CEE,
        end: 0x1CF3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CF4,
        end: 0x1CF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CF5,
        end: 0x1CF6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CF7,
        end: 0x1CF7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CF8,
        end: 0x1CF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1CFA,
        end: 0x1CFA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D00,
        end: 0x1D2B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D2C,
        end: 0x1D6A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D6B,
        end: 0x1D77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D78,
        end: 0x1D78,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D79,
        end: 0x1D7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D80,
        end: 0x1D9A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1D9B,
        end: 0x1DBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1DC0,
        end: 0x1DFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1E00,
        end: 0x1EFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F00,
        end: 0x1F15,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F18,
        end: 0x1F1D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F20,
        end: 0x1F45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F48,
        end: 0x1F4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F50,
        end: 0x1F57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F59,
        end: 0x1F59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F5B,
        end: 0x1F5B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F5D,
        end: 0x1F5D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F5F,
        end: 0x1F7D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1F80,
        end: 0x1FB4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FB6,
        end: 0x1FBC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FBD,
        end: 0x1FBD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FBE,
        end: 0x1FBE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FBF,
        end: 0x1FC1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FC2,
        end: 0x1FC4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FC6,
        end: 0x1FCC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FCD,
        end: 0x1FCF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FD0,
        end: 0x1FD3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FD6,
        end: 0x1FDB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FDD,
        end: 0x1FDF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FE0,
        end: 0x1FEC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FED,
        end: 0x1FEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FF2,
        end: 0x1FF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FF6,
        end: 0x1FFC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x1FFD,
        end: 0x1FFE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2000,
        end: 0x200A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x200B,
        end: 0x200F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2010,
        end: 0x2015,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2016,
        end: 0x2016,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2017,
        end: 0x2017,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2018,
        end: 0x2018,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x2019,
        end: 0x2019,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x201A,
        end: 0x201A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x201B,
        end: 0x201B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x201C,
        end: 0x201C,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x201D,
        end: 0x201D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x201E,
        end: 0x201E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x201F,
        end: 0x201F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2020,
        end: 0x2021,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2022,
        end: 0x2027,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2028,
        end: 0x2028,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2029,
        end: 0x2029,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x202A,
        end: 0x202E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x202F,
        end: 0x202F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2030,
        end: 0x2031,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2032,
        end: 0x2038,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2039,
        end: 0x2039,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x203A,
        end: 0x203A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x203B,
        end: 0x203C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x203D,
        end: 0x203E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x203F,
        end: 0x2040,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2041,
        end: 0x2041,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2042,
        end: 0x2042,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2043,
        end: 0x2043,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2044,
        end: 0x2044,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2045,
        end: 0x2045,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2046,
        end: 0x2046,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2047,
        end: 0x2049,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x204A,
        end: 0x2050,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2051,
        end: 0x2051,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2052,
        end: 0x2052,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2053,
        end: 0x2053,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2054,
        end: 0x2054,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2055,
        end: 0x205E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x205F,
        end: 0x205F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2060,
        end: 0x2064,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2065,
        end: 0x2065,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2066,
        end: 0x206F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2070,
        end: 0x2070,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2071,
        end: 0x2071,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2074,
        end: 0x2079,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x207A,
        end: 0x207C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x207D,
        end: 0x207D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x207E,
        end: 0x207E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x207F,
        end: 0x207F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2080,
        end: 0x2089,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x208A,
        end: 0x208C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x208D,
        end: 0x208D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x208E,
        end: 0x208E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2090,
        end: 0x209C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x20A0,
        end: 0x20C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x20D0,
        end: 0x20DC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x20DD,
        end: 0x20E0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x20E1,
        end: 0x20E1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x20E2,
        end: 0x20E4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x20E5,
        end: 0x20F0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2100,
        end: 0x2101,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2102,
        end: 0x2102,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2103,
        end: 0x2106,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2107,
        end: 0x2107,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2108,
        end: 0x2109,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x210A,
        end: 0x210E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x210F,
        end: 0x210F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2110,
        end: 0x2112,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2113,
        end: 0x2113,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2114,
        end: 0x2114,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2115,
        end: 0x2115,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2116,
        end: 0x2117,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2118,
        end: 0x2118,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2119,
        end: 0x211D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x211E,
        end: 0x2123,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2124,
        end: 0x2124,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2125,
        end: 0x2125,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2126,
        end: 0x2126,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2127,
        end: 0x2127,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2128,
        end: 0x2128,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2129,
        end: 0x2129,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x212A,
        end: 0x212D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x212E,
        end: 0x212E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x212F,
        end: 0x2134,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2135,
        end: 0x2138,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2139,
        end: 0x2139,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x213A,
        end: 0x213B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x213C,
        end: 0x213F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2140,
        end: 0x2144,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2145,
        end: 0x2149,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x214A,
        end: 0x214A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x214B,
        end: 0x214B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x214C,
        end: 0x214D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x214E,
        end: 0x214E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x214F,
        end: 0x214F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2150,
        end: 0x215F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2160,
        end: 0x2182,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2183,
        end: 0x2184,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2185,
        end: 0x2188,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2189,
        end: 0x2189,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x218A,
        end: 0x218B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x218C,
        end: 0x218F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2190,
        end: 0x2194,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2195,
        end: 0x2199,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x219A,
        end: 0x219B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x219C,
        end: 0x219F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A0,
        end: 0x21A0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A1,
        end: 0x21A2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A3,
        end: 0x21A3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A4,
        end: 0x21A5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A6,
        end: 0x21A6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21A7,
        end: 0x21AD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21AE,
        end: 0x21AE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21AF,
        end: 0x21CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21CE,
        end: 0x21CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21D0,
        end: 0x21D1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21D2,
        end: 0x21D2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21D3,
        end: 0x21D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21D4,
        end: 0x21D4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21D5,
        end: 0x21F3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x21F4,
        end: 0x21FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2200,
        end: 0x221D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x221E,
        end: 0x221E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x221F,
        end: 0x2233,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2234,
        end: 0x2235,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2236,
        end: 0x22FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2300,
        end: 0x2307,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2308,
        end: 0x2308,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2309,
        end: 0x2309,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x230A,
        end: 0x230A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x230B,
        end: 0x230B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x230C,
        end: 0x231F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2320,
        end: 0x2321,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2322,
        end: 0x2323,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2324,
        end: 0x2328,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2329,
        end: 0x2329,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x232A,
        end: 0x232A,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x232B,
        end: 0x232B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x232C,
        end: 0x237B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x237C,
        end: 0x237C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x237D,
        end: 0x239A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x239B,
        end: 0x23B3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x23B4,
        end: 0x23BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x23BE,
        end: 0x23CD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x23CE,
        end: 0x23CE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x23CF,
        end: 0x23CF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x23D0,
        end: 0x23D0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x23D1,
        end: 0x23DB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x23DC,
        end: 0x23E1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x23E2,
        end: 0x23FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2400,
        end: 0x2422,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2423,
        end: 0x2423,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2424,
        end: 0x2429,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x242A,
        end: 0x243F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2440,
        end: 0x244A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x244B,
        end: 0x245F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2460,
        end: 0x249B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x249C,
        end: 0x24E9,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x24EA,
        end: 0x24FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2500,
        end: 0x257F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2580,
        end: 0x259F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x25A0,
        end: 0x25B6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x25B7,
        end: 0x25B7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x25B8,
        end: 0x25C0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x25C1,
        end: 0x25C1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x25C2,
        end: 0x25F7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x25F8,
        end: 0x25FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2600,
        end: 0x2619,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x261A,
        end: 0x261F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2620,
        end: 0x266E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x266F,
        end: 0x266F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2670,
        end: 0x26FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2700,
        end: 0x2767,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2768,
        end: 0x2768,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2769,
        end: 0x2769,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276A,
        end: 0x276A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276B,
        end: 0x276B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276C,
        end: 0x276C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276D,
        end: 0x276D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276E,
        end: 0x276E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x276F,
        end: 0x276F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2770,
        end: 0x2770,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2771,
        end: 0x2771,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2772,
        end: 0x2772,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2773,
        end: 0x2773,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2774,
        end: 0x2774,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2775,
        end: 0x2775,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2776,
        end: 0x2793,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2794,
        end: 0x27BF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27C0,
        end: 0x27C4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27C5,
        end: 0x27C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27C6,
        end: 0x27C6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27C7,
        end: 0x27E5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27E6,
        end: 0x27E6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27E7,
        end: 0x27E7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27E8,
        end: 0x27E8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27E9,
        end: 0x27E9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27EA,
        end: 0x27EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27EB,
        end: 0x27EB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27EC,
        end: 0x27EC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27ED,
        end: 0x27ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27EE,
        end: 0x27EE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27EF,
        end: 0x27EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x27F0,
        end: 0x27FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2800,
        end: 0x28FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2900,
        end: 0x297F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2980,
        end: 0x2982,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2983,
        end: 0x2983,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2984,
        end: 0x2984,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2985,
        end: 0x2985,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2986,
        end: 0x2986,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2987,
        end: 0x2987,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2988,
        end: 0x2988,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2989,
        end: 0x2989,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298A,
        end: 0x298A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298B,
        end: 0x298B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298C,
        end: 0x298C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298D,
        end: 0x298D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298E,
        end: 0x298E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x298F,
        end: 0x298F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2990,
        end: 0x2990,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2991,
        end: 0x2991,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2992,
        end: 0x2992,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2993,
        end: 0x2993,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2994,
        end: 0x2994,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2995,
        end: 0x2995,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2996,
        end: 0x2996,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2997,
        end: 0x2997,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2998,
        end: 0x2998,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2999,
        end: 0x29D7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29D8,
        end: 0x29D8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29D9,
        end: 0x29D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29DA,
        end: 0x29DA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29DB,
        end: 0x29DB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29DC,
        end: 0x29FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29FC,
        end: 0x29FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29FD,
        end: 0x29FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x29FE,
        end: 0x29FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2A00,
        end: 0x2AFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B00,
        end: 0x2B11,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B12,
        end: 0x2B2F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2B30,
        end: 0x2B44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B45,
        end: 0x2B46,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B47,
        end: 0x2B4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B4D,
        end: 0x2B4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B50,
        end: 0x2B59,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2B5A,
        end: 0x2B73,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B76,
        end: 0x2B96,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2B97,
        end: 0x2B97,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2B98,
        end: 0x2BB7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2BB8,
        end: 0x2BD1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2BD2,
        end: 0x2BD2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2BD3,
        end: 0x2BEB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2BEC,
        end: 0x2BEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2BF0,
        end: 0x2BFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2C00,
        end: 0x2C5F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2C60,
        end: 0x2C7B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2C7C,
        end: 0x2C7D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2C7E,
        end: 0x2C7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2C80,
        end: 0x2CE4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CE5,
        end: 0x2CEA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CEB,
        end: 0x2CEE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CEF,
        end: 0x2CF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CF2,
        end: 0x2CF3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CF9,
        end: 0x2CFC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CFD,
        end: 0x2CFD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2CFE,
        end: 0x2CFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D00,
        end: 0x2D25,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D27,
        end: 0x2D27,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D2D,
        end: 0x2D2D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D30,
        end: 0x2D67,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D6F,
        end: 0x2D6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D70,
        end: 0x2D70,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D7F,
        end: 0x2D7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2D80,
        end: 0x2D96,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DA0,
        end: 0x2DA6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DA8,
        end: 0x2DAE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DB0,
        end: 0x2DB6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DB8,
        end: 0x2DBE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DC0,
        end: 0x2DC6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DC8,
        end: 0x2DCE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DD0,
        end: 0x2DD6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DD8,
        end: 0x2DDE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2DE0,
        end: 0x2DFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E00,
        end: 0x2E01,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E02,
        end: 0x2E02,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E03,
        end: 0x2E03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E04,
        end: 0x2E04,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E05,
        end: 0x2E05,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E06,
        end: 0x2E08,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E09,
        end: 0x2E09,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E0A,
        end: 0x2E0A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E0B,
        end: 0x2E0B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E0C,
        end: 0x2E0C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E0D,
        end: 0x2E0D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E0E,
        end: 0x2E16,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E17,
        end: 0x2E17,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E18,
        end: 0x2E19,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E1A,
        end: 0x2E1A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E1B,
        end: 0x2E1B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E1C,
        end: 0x2E1C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E1D,
        end: 0x2E1D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E1E,
        end: 0x2E1F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E20,
        end: 0x2E20,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E21,
        end: 0x2E21,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E22,
        end: 0x2E22,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E23,
        end: 0x2E23,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E24,
        end: 0x2E24,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E25,
        end: 0x2E25,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E26,
        end: 0x2E26,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E27,
        end: 0x2E27,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E28,
        end: 0x2E28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E29,
        end: 0x2E29,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E2A,
        end: 0x2E2E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E2F,
        end: 0x2E2F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E30,
        end: 0x2E39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E3A,
        end: 0x2E3B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E3C,
        end: 0x2E3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E40,
        end: 0x2E40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E41,
        end: 0x2E41,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E42,
        end: 0x2E42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E43,
        end: 0x2E4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E50,
        end: 0x2E51,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2E52,
        end: 0x2E54,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E55,
        end: 0x2E55,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E56,
        end: 0x2E56,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E57,
        end: 0x2E57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E58,
        end: 0x2E58,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E59,
        end: 0x2E59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E5A,
        end: 0x2E5A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E5B,
        end: 0x2E5B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E5C,
        end: 0x2E5C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E5D,
        end: 0x2E5D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x2E80,
        end: 0x2E99,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2E9A,
        end: 0x2E9A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2E9B,
        end: 0x2EF3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2EF4,
        end: 0x2EFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2F00,
        end: 0x2FD5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2FD6,
        end: 0x2FDF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2FE0,
        end: 0x2FEF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x2FF0,
        end: 0x2FFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3000,
        end: 0x3000,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3001,
        end: 0x3002,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3003,
        end: 0x3003,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3004,
        end: 0x3004,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3005,
        end: 0x3005,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3006,
        end: 0x3006,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3007,
        end: 0x3007,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3008,
        end: 0x3008,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3009,
        end: 0x3009,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300A,
        end: 0x300A,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300B,
        end: 0x300B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300C,
        end: 0x300C,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300D,
        end: 0x300D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300E,
        end: 0x300E,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x300F,
        end: 0x300F,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3010,
        end: 0x3010,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3011,
        end: 0x3011,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3012,
        end: 0x3013,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3014,
        end: 0x3014,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3015,
        end: 0x3015,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3016,
        end: 0x3016,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3017,
        end: 0x3017,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3018,
        end: 0x3018,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3019,
        end: 0x3019,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x301A,
        end: 0x301A,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x301B,
        end: 0x301B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x301C,
        end: 0x301C,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x301D,
        end: 0x301D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x301E,
        end: 0x301F,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3020,
        end: 0x3020,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3021,
        end: 0x3029,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x302A,
        end: 0x302D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x302E,
        end: 0x302F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3030,
        end: 0x3030,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x3031,
        end: 0x3035,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3036,
        end: 0x3037,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3038,
        end: 0x303A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x303B,
        end: 0x303B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x303C,
        end: 0x303C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x303D,
        end: 0x303D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x303E,
        end: 0x303F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3040,
        end: 0x3040,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3041,
        end: 0x3041,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3042,
        end: 0x3042,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3043,
        end: 0x3043,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3044,
        end: 0x3044,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3045,
        end: 0x3045,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3046,
        end: 0x3046,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3047,
        end: 0x3047,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3048,
        end: 0x3048,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3049,
        end: 0x3049,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x304A,
        end: 0x3062,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3063,
        end: 0x3063,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3064,
        end: 0x3082,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3083,
        end: 0x3083,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3084,
        end: 0x3084,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3085,
        end: 0x3085,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3086,
        end: 0x3086,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3087,
        end: 0x3087,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3088,
        end: 0x308D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x308E,
        end: 0x308E,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x308F,
        end: 0x3094,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3095,
        end: 0x3096,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3097,
        end: 0x3098,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3099,
        end: 0x309A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x309B,
        end: 0x309C,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x309D,
        end: 0x309E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x309F,
        end: 0x309F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30A0,
        end: 0x30A0,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x30A1,
        end: 0x30A1,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30A2,
        end: 0x30A2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30A3,
        end: 0x30A3,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30A4,
        end: 0x30A4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30A5,
        end: 0x30A5,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30A6,
        end: 0x30A6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30A7,
        end: 0x30A7,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30A8,
        end: 0x30A8,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30A9,
        end: 0x30A9,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30AA,
        end: 0x30C2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30C3,
        end: 0x30C3,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30C4,
        end: 0x30E2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30E3,
        end: 0x30E3,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30E4,
        end: 0x30E4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30E5,
        end: 0x30E5,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30E6,
        end: 0x30E6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30E7,
        end: 0x30E7,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30E8,
        end: 0x30ED,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30EE,
        end: 0x30EE,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30EF,
        end: 0x30F4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30F5,
        end: 0x30F6,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x30F7,
        end: 0x30FA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30FB,
        end: 0x30FB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30FC,
        end: 0x30FC,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0x30FD,
        end: 0x30FE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x30FF,
        end: 0x30FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3100,
        end: 0x3104,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3105,
        end: 0x3126,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3127,
        end: 0x3127,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3128,
        end: 0x312F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3130,
        end: 0x3130,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3131,
        end: 0x318E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x318F,
        end: 0x318F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3190,
        end: 0x3191,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3192,
        end: 0x3195,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3196,
        end: 0x319F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31A0,
        end: 0x31B3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31B4,
        end: 0x31B7,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x31B8,
        end: 0x31BA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31BB,
        end: 0x31BB,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x31BC,
        end: 0x31BF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31C0,
        end: 0x31E5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31E6,
        end: 0x31EE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31EF,
        end: 0x31EF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x31F0,
        end: 0x31FF,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3200,
        end: 0x321E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x321F,
        end: 0x321F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3220,
        end: 0x3229,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x322A,
        end: 0x3247,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3248,
        end: 0x324F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3250,
        end: 0x3250,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3251,
        end: 0x325F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3260,
        end: 0x327F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3280,
        end: 0x3289,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x328A,
        end: 0x32B0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x32B1,
        end: 0x32BF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x32C0,
        end: 0x32FE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x32FF,
        end: 0x32FF,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3300,
        end: 0x3357,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3358,
        end: 0x337A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x337B,
        end: 0x337F,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x3380,
        end: 0x33FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x3400,
        end: 0x4DBF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x4DC0,
        end: 0x4DFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x4E00,
        end: 0x9FFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA000,
        end: 0xA014,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA015,
        end: 0xA015,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA016,
        end: 0xA48C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA48D,
        end: 0xA48F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA490,
        end: 0xA4C6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA4C7,
        end: 0xA4CF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA4D0,
        end: 0xA4F7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA4F8,
        end: 0xA4FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA4FE,
        end: 0xA4FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA500,
        end: 0xA60B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA60C,
        end: 0xA60C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA60D,
        end: 0xA60F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA610,
        end: 0xA61F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA620,
        end: 0xA629,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA62A,
        end: 0xA62B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA640,
        end: 0xA66D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA66E,
        end: 0xA66E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA66F,
        end: 0xA66F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA670,
        end: 0xA672,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA673,
        end: 0xA673,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA674,
        end: 0xA67D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA67E,
        end: 0xA67E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA67F,
        end: 0xA67F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA680,
        end: 0xA69B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA69C,
        end: 0xA69D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA69E,
        end: 0xA69F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA6A0,
        end: 0xA6E5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA6E6,
        end: 0xA6EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA6F0,
        end: 0xA6F1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA6F2,
        end: 0xA6F7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA700,
        end: 0xA716,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA717,
        end: 0xA71F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA720,
        end: 0xA721,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA722,
        end: 0xA76F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA770,
        end: 0xA770,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA771,
        end: 0xA787,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA788,
        end: 0xA788,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA789,
        end: 0xA78A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA78B,
        end: 0xA78E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA78F,
        end: 0xA78F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA790,
        end: 0xA7DC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7F1,
        end: 0xA7F4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7F5,
        end: 0xA7F6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7F7,
        end: 0xA7F7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7F8,
        end: 0xA7F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7FA,
        end: 0xA7FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA7FB,
        end: 0xA7FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA800,
        end: 0xA801,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA802,
        end: 0xA802,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA803,
        end: 0xA805,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA806,
        end: 0xA806,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA807,
        end: 0xA80A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA80B,
        end: 0xA80B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA80C,
        end: 0xA822,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA823,
        end: 0xA824,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA825,
        end: 0xA826,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA827,
        end: 0xA827,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA828,
        end: 0xA82B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA82C,
        end: 0xA82C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA830,
        end: 0xA835,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA836,
        end: 0xA837,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA838,
        end: 0xA838,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA839,
        end: 0xA839,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA840,
        end: 0xA873,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA874,
        end: 0xA877,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA880,
        end: 0xA881,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA882,
        end: 0xA8B3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8B4,
        end: 0xA8C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8C4,
        end: 0xA8C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8CE,
        end: 0xA8CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8D0,
        end: 0xA8D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8E0,
        end: 0xA8F1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8F2,
        end: 0xA8F7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8F8,
        end: 0xA8FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8FB,
        end: 0xA8FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8FC,
        end: 0xA8FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8FD,
        end: 0xA8FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA8FF,
        end: 0xA8FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA900,
        end: 0xA909,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA90A,
        end: 0xA925,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA926,
        end: 0xA92D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA92E,
        end: 0xA92F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA930,
        end: 0xA946,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA947,
        end: 0xA951,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA952,
        end: 0xA953,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA95F,
        end: 0xA95F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA960,
        end: 0xA97C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA97D,
        end: 0xA97F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xA980,
        end: 0xA982,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA983,
        end: 0xA983,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA984,
        end: 0xA9B2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9B3,
        end: 0xA9B3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9B4,
        end: 0xA9B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9B6,
        end: 0xA9B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9BA,
        end: 0xA9BB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9BC,
        end: 0xA9BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9BE,
        end: 0xA9C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9C1,
        end: 0xA9CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9CF,
        end: 0xA9CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9D0,
        end: 0xA9D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9DE,
        end: 0xA9DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9E0,
        end: 0xA9E4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9E5,
        end: 0xA9E5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9E6,
        end: 0xA9E6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9E7,
        end: 0xA9EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9F0,
        end: 0xA9F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xA9FA,
        end: 0xA9FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA00,
        end: 0xAA28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA29,
        end: 0xAA2E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA2F,
        end: 0xAA30,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA31,
        end: 0xAA32,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA33,
        end: 0xAA34,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA35,
        end: 0xAA36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA40,
        end: 0xAA42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA43,
        end: 0xAA43,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA44,
        end: 0xAA4B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA4C,
        end: 0xAA4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA4D,
        end: 0xAA4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA50,
        end: 0xAA59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA5C,
        end: 0xAA5F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA60,
        end: 0xAA6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA70,
        end: 0xAA70,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA71,
        end: 0xAA76,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA77,
        end: 0xAA79,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA7A,
        end: 0xAA7A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA7B,
        end: 0xAA7B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA7C,
        end: 0xAA7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA7D,
        end: 0xAA7D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA7E,
        end: 0xAA7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAA80,
        end: 0xAAAF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB0,
        end: 0xAAB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB1,
        end: 0xAAB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB2,
        end: 0xAAB4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB5,
        end: 0xAAB6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB7,
        end: 0xAAB8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAB9,
        end: 0xAABD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAABE,
        end: 0xAABF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAC0,
        end: 0xAAC0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAC1,
        end: 0xAAC1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAC2,
        end: 0xAAC2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAADB,
        end: 0xAADC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAADD,
        end: 0xAADD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAADE,
        end: 0xAADF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAE0,
        end: 0xAAEA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAEB,
        end: 0xAAEB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAEC,
        end: 0xAAED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAEE,
        end: 0xAAEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAF0,
        end: 0xAAF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAF2,
        end: 0xAAF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAF3,
        end: 0xAAF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAF5,
        end: 0xAAF5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAAF6,
        end: 0xAAF6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB01,
        end: 0xAB06,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB09,
        end: 0xAB0E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB11,
        end: 0xAB16,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB20,
        end: 0xAB26,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB28,
        end: 0xAB2E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB30,
        end: 0xAB5A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB5B,
        end: 0xAB5B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB5C,
        end: 0xAB5F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB60,
        end: 0xAB68,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB69,
        end: 0xAB69,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB6A,
        end: 0xAB6B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAB70,
        end: 0xABBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABC0,
        end: 0xABE2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABE3,
        end: 0xABE4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABE5,
        end: 0xABE5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABE6,
        end: 0xABE7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABE8,
        end: 0xABE8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABE9,
        end: 0xABEA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABEB,
        end: 0xABEB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABEC,
        end: 0xABEC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABED,
        end: 0xABED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xABF0,
        end: 0xABF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xAC00,
        end: 0xD7A3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD7A4,
        end: 0xD7AF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD7B0,
        end: 0xD7C6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD7C7,
        end: 0xD7CA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD7CB,
        end: 0xD7FB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD7FC,
        end: 0xD7FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xD800,
        end: 0xDB7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xDB80,
        end: 0xDBFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xDC00,
        end: 0xDFFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xE000,
        end: 0xF8FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xF900,
        end: 0xFA6D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFA6E,
        end: 0xFA6F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFA70,
        end: 0xFAD9,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFADA,
        end: 0xFAFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFB00,
        end: 0xFB06,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB13,
        end: 0xFB17,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB1D,
        end: 0xFB1D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB1E,
        end: 0xFB1E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB1F,
        end: 0xFB28,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB29,
        end: 0xFB29,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB2A,
        end: 0xFB36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB38,
        end: 0xFB3C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB3E,
        end: 0xFB3E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB40,
        end: 0xFB41,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB43,
        end: 0xFB44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB46,
        end: 0xFB4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFB50,
        end: 0xFBB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFBB2,
        end: 0xFBC2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFBC3,
        end: 0xFBD2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFBD3,
        end: 0xFD3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD3E,
        end: 0xFD3E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD3F,
        end: 0xFD3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD40,
        end: 0xFD4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD50,
        end: 0xFD8F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD90,
        end: 0xFD91,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFD92,
        end: 0xFDC7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFDC8,
        end: 0xFDCF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFDF0,
        end: 0xFDFB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFDFC,
        end: 0xFDFC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFDFD,
        end: 0xFDFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE00,
        end: 0xFE0F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE10,
        end: 0xFE16,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE17,
        end: 0xFE17,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE18,
        end: 0xFE18,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE19,
        end: 0xFE19,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE1A,
        end: 0xFE1F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE20,
        end: 0xFE2F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE30,
        end: 0xFE30,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE31,
        end: 0xFE32,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE33,
        end: 0xFE34,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE35,
        end: 0xFE35,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE36,
        end: 0xFE36,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE37,
        end: 0xFE37,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE38,
        end: 0xFE38,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE39,
        end: 0xFE39,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3A,
        end: 0xFE3A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3B,
        end: 0xFE3B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3C,
        end: 0xFE3C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3D,
        end: 0xFE3D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3E,
        end: 0xFE3E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE3F,
        end: 0xFE3F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE40,
        end: 0xFE40,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE41,
        end: 0xFE41,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE42,
        end: 0xFE42,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE43,
        end: 0xFE43,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE44,
        end: 0xFE44,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE45,
        end: 0xFE46,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE47,
        end: 0xFE47,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE48,
        end: 0xFE48,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE49,
        end: 0xFE4C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE4D,
        end: 0xFE4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE50,
        end: 0xFE52,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0xFE53,
        end: 0xFE53,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE54,
        end: 0xFE57,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE58,
        end: 0xFE58,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE59,
        end: 0xFE59,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5A,
        end: 0xFE5A,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5B,
        end: 0xFE5B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5C,
        end: 0xFE5C,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5D,
        end: 0xFE5D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5E,
        end: 0xFE5E,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFE5F,
        end: 0xFE61,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE62,
        end: 0xFE62,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE63,
        end: 0xFE63,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE64,
        end: 0xFE66,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE67,
        end: 0xFE67,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE68,
        end: 0xFE68,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE69,
        end: 0xFE69,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE6A,
        end: 0xFE6B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE6C,
        end: 0xFE6F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFE70,
        end: 0xFE74,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFE76,
        end: 0xFEFC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFEFF,
        end: 0xFEFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF01,
        end: 0xFF01,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0xFF02,
        end: 0xFF03,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF04,
        end: 0xFF04,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF05,
        end: 0xFF07,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF08,
        end: 0xFF08,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF09,
        end: 0xFF09,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF0A,
        end: 0xFF0A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF0B,
        end: 0xFF0B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF0C,
        end: 0xFF0C,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0xFF0D,
        end: 0xFF0D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF0E,
        end: 0xFF0E,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0xFF0F,
        end: 0xFF0F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF10,
        end: 0xFF19,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF1A,
        end: 0xFF1B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF1C,
        end: 0xFF1E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF1F,
        end: 0xFF1F,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0xFF20,
        end: 0xFF20,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF21,
        end: 0xFF3A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF3B,
        end: 0xFF3B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF3C,
        end: 0xFF3C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF3D,
        end: 0xFF3D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF3E,
        end: 0xFF3E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF3F,
        end: 0xFF3F,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF40,
        end: 0xFF40,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF41,
        end: 0xFF5A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFF5B,
        end: 0xFF5B,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF5C,
        end: 0xFF5C,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF5D,
        end: 0xFF5D,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF5E,
        end: 0xFF5E,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF5F,
        end: 0xFF5F,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF60,
        end: 0xFF60,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFF61,
        end: 0xFF61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF62,
        end: 0xFF62,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF63,
        end: 0xFF63,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF64,
        end: 0xFF65,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF66,
        end: 0xFF6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF70,
        end: 0xFF70,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF71,
        end: 0xFF9D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFF9E,
        end: 0xFF9F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFA0,
        end: 0xFFBE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFC2,
        end: 0xFFC7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFCA,
        end: 0xFFCF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFD2,
        end: 0xFFD7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFDA,
        end: 0xFFDC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFE0,
        end: 0xFFE1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFE2,
        end: 0xFFE2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFE3,
        end: 0xFFE3,
        orientation: UnicodeVerticalOrientation::TransformedRotated,
    },
    VerticalOrientationRange {
        start: 0xFFE4,
        end: 0xFFE4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFE5,
        end: 0xFFE6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFE7,
        end: 0xFFE7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFE8,
        end: 0xFFE8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFE9,
        end: 0xFFEC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFED,
        end: 0xFFEE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFF0,
        end: 0xFFF8,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0xFFF9,
        end: 0xFFFB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0xFFFC,
        end: 0xFFFD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_0000,
        end: 0x0001_000B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_000D,
        end: 0x0001_0026,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0028,
        end: 0x0001_003A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_003C,
        end: 0x0001_003D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_003F,
        end: 0x0001_004D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0050,
        end: 0x0001_005D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0080,
        end: 0x0001_00FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0100,
        end: 0x0001_0102,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0107,
        end: 0x0001_0133,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0137,
        end: 0x0001_013F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0140,
        end: 0x0001_0174,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0175,
        end: 0x0001_0178,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0179,
        end: 0x0001_0189,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_018A,
        end: 0x0001_018B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_018C,
        end: 0x0001_018E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0190,
        end: 0x0001_019C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_01A0,
        end: 0x0001_01A0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_01D0,
        end: 0x0001_01FC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_01FD,
        end: 0x0001_01FD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0280,
        end: 0x0001_029C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_02A0,
        end: 0x0001_02D0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_02E0,
        end: 0x0001_02E0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_02E1,
        end: 0x0001_02FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0300,
        end: 0x0001_031F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0320,
        end: 0x0001_0323,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_032D,
        end: 0x0001_032F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0330,
        end: 0x0001_0340,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0341,
        end: 0x0001_0341,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0342,
        end: 0x0001_0349,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_034A,
        end: 0x0001_034A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0350,
        end: 0x0001_0375,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0376,
        end: 0x0001_037A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0380,
        end: 0x0001_039D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_039F,
        end: 0x0001_039F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_03A0,
        end: 0x0001_03C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_03C8,
        end: 0x0001_03CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_03D0,
        end: 0x0001_03D0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_03D1,
        end: 0x0001_03D5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0400,
        end: 0x0001_044F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0450,
        end: 0x0001_047F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0480,
        end: 0x0001_049D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_04A0,
        end: 0x0001_04A9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_04B0,
        end: 0x0001_04D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_04D8,
        end: 0x0001_04FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0500,
        end: 0x0001_0527,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0530,
        end: 0x0001_0563,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_056F,
        end: 0x0001_056F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0570,
        end: 0x0001_057A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_057C,
        end: 0x0001_058A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_058C,
        end: 0x0001_0592,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0594,
        end: 0x0001_0595,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0597,
        end: 0x0001_05A1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_05A3,
        end: 0x0001_05B1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_05B3,
        end: 0x0001_05B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_05BB,
        end: 0x0001_05BC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_05C0,
        end: 0x0001_05F3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0600,
        end: 0x0001_0736,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0740,
        end: 0x0001_0755,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0760,
        end: 0x0001_0767,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0780,
        end: 0x0001_0785,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0787,
        end: 0x0001_07B0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_07B2,
        end: 0x0001_07BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0800,
        end: 0x0001_0805,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0808,
        end: 0x0001_0808,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_080A,
        end: 0x0001_0835,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0837,
        end: 0x0001_0838,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_083C,
        end: 0x0001_083C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_083F,
        end: 0x0001_083F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0840,
        end: 0x0001_0855,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0857,
        end: 0x0001_0857,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0858,
        end: 0x0001_085F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0860,
        end: 0x0001_0876,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0877,
        end: 0x0001_0878,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0879,
        end: 0x0001_087F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0880,
        end: 0x0001_089E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_08A7,
        end: 0x0001_08AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_08E0,
        end: 0x0001_08F2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_08F4,
        end: 0x0001_08F5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_08FB,
        end: 0x0001_08FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0900,
        end: 0x0001_0915,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0916,
        end: 0x0001_091B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_091F,
        end: 0x0001_091F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0920,
        end: 0x0001_0939,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_093F,
        end: 0x0001_093F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0940,
        end: 0x0001_0959,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0980,
        end: 0x0001_099F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_09A0,
        end: 0x0001_09B7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_09BC,
        end: 0x0001_09BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_09BE,
        end: 0x0001_09BF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_09C0,
        end: 0x0001_09CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_09D2,
        end: 0x0001_09FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A00,
        end: 0x0001_0A00,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A01,
        end: 0x0001_0A03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A05,
        end: 0x0001_0A06,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A0C,
        end: 0x0001_0A0F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A10,
        end: 0x0001_0A13,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A15,
        end: 0x0001_0A17,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A19,
        end: 0x0001_0A35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A38,
        end: 0x0001_0A3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A3F,
        end: 0x0001_0A3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A40,
        end: 0x0001_0A48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A50,
        end: 0x0001_0A58,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A60,
        end: 0x0001_0A7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A7D,
        end: 0x0001_0A7E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A7F,
        end: 0x0001_0A7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A80,
        end: 0x0001_0A9C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0A9D,
        end: 0x0001_0A9F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AC0,
        end: 0x0001_0AC7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AC8,
        end: 0x0001_0AC8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AC9,
        end: 0x0001_0AE4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AE5,
        end: 0x0001_0AE6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AEB,
        end: 0x0001_0AEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0AF0,
        end: 0x0001_0AF6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B00,
        end: 0x0001_0B35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B39,
        end: 0x0001_0B3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B40,
        end: 0x0001_0B55,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B58,
        end: 0x0001_0B5F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B60,
        end: 0x0001_0B72,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B78,
        end: 0x0001_0B7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B80,
        end: 0x0001_0B91,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0B99,
        end: 0x0001_0B9C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0BA9,
        end: 0x0001_0BAF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0C00,
        end: 0x0001_0C48,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0C80,
        end: 0x0001_0CB2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0CC0,
        end: 0x0001_0CF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0CFA,
        end: 0x0001_0CFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D00,
        end: 0x0001_0D23,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D24,
        end: 0x0001_0D27,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D30,
        end: 0x0001_0D39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D40,
        end: 0x0001_0D49,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D4A,
        end: 0x0001_0D4D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D4E,
        end: 0x0001_0D4E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D4F,
        end: 0x0001_0D4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D50,
        end: 0x0001_0D65,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D69,
        end: 0x0001_0D6D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D6E,
        end: 0x0001_0D6E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D6F,
        end: 0x0001_0D6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D70,
        end: 0x0001_0D85,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0D8E,
        end: 0x0001_0D8F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0E60,
        end: 0x0001_0E7E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0E80,
        end: 0x0001_0EA9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EAB,
        end: 0x0001_0EAC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EAD,
        end: 0x0001_0EAD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EB0,
        end: 0x0001_0EB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EC2,
        end: 0x0001_0EC4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EC5,
        end: 0x0001_0EC5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EC6,
        end: 0x0001_0EC7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0ED0,
        end: 0x0001_0ED0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0ED1,
        end: 0x0001_0ED8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0EFA,
        end: 0x0001_0EFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F00,
        end: 0x0001_0F1C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F1D,
        end: 0x0001_0F26,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F27,
        end: 0x0001_0F27,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F30,
        end: 0x0001_0F45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F46,
        end: 0x0001_0F50,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F51,
        end: 0x0001_0F54,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F55,
        end: 0x0001_0F59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F70,
        end: 0x0001_0F81,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F82,
        end: 0x0001_0F85,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0F86,
        end: 0x0001_0F89,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0FB0,
        end: 0x0001_0FC4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0FC5,
        end: 0x0001_0FCB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_0FE0,
        end: 0x0001_0FF6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1000,
        end: 0x0001_1000,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1001,
        end: 0x0001_1001,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1002,
        end: 0x0001_1002,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1003,
        end: 0x0001_1037,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1038,
        end: 0x0001_1046,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1047,
        end: 0x0001_104D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1052,
        end: 0x0001_1065,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1066,
        end: 0x0001_106F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1070,
        end: 0x0001_1070,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1071,
        end: 0x0001_1072,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1073,
        end: 0x0001_1074,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1075,
        end: 0x0001_1075,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_107F,
        end: 0x0001_107F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1080,
        end: 0x0001_1081,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1082,
        end: 0x0001_1082,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1083,
        end: 0x0001_10AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10B0,
        end: 0x0001_10B2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10B3,
        end: 0x0001_10B6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10B7,
        end: 0x0001_10B8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10B9,
        end: 0x0001_10BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10BB,
        end: 0x0001_10BC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10BD,
        end: 0x0001_10BD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10BE,
        end: 0x0001_10C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10C2,
        end: 0x0001_10C2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10CD,
        end: 0x0001_10CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10D0,
        end: 0x0001_10E8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_10F0,
        end: 0x0001_10F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1100,
        end: 0x0001_1102,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1103,
        end: 0x0001_1126,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1127,
        end: 0x0001_112B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_112C,
        end: 0x0001_112C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_112D,
        end: 0x0001_1134,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1136,
        end: 0x0001_113F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1140,
        end: 0x0001_1143,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1144,
        end: 0x0001_1144,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1145,
        end: 0x0001_1146,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1147,
        end: 0x0001_1147,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1150,
        end: 0x0001_1172,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1173,
        end: 0x0001_1173,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1174,
        end: 0x0001_1175,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1176,
        end: 0x0001_1176,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1180,
        end: 0x0001_1181,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1182,
        end: 0x0001_1182,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1183,
        end: 0x0001_11B2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11B3,
        end: 0x0001_11B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11B6,
        end: 0x0001_11BE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11BF,
        end: 0x0001_11C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11C1,
        end: 0x0001_11C4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11C5,
        end: 0x0001_11C8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11C9,
        end: 0x0001_11CC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11CD,
        end: 0x0001_11CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11CE,
        end: 0x0001_11CE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11CF,
        end: 0x0001_11CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11D0,
        end: 0x0001_11D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11DA,
        end: 0x0001_11DA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11DB,
        end: 0x0001_11DB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11DC,
        end: 0x0001_11DC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11DD,
        end: 0x0001_11DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_11E1,
        end: 0x0001_11F4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1200,
        end: 0x0001_1211,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1213,
        end: 0x0001_122B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_122C,
        end: 0x0001_122E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_122F,
        end: 0x0001_1231,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1232,
        end: 0x0001_1233,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1234,
        end: 0x0001_1234,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1235,
        end: 0x0001_1235,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1236,
        end: 0x0001_1237,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1238,
        end: 0x0001_123D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_123E,
        end: 0x0001_123E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_123F,
        end: 0x0001_1240,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1241,
        end: 0x0001_1241,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1280,
        end: 0x0001_1286,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1288,
        end: 0x0001_1288,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_128A,
        end: 0x0001_128D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_128F,
        end: 0x0001_129D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_129F,
        end: 0x0001_12A8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12A9,
        end: 0x0001_12A9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12B0,
        end: 0x0001_12DE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12DF,
        end: 0x0001_12DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12E0,
        end: 0x0001_12E2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12E3,
        end: 0x0001_12EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_12F0,
        end: 0x0001_12F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1300,
        end: 0x0001_1301,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1302,
        end: 0x0001_1303,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1305,
        end: 0x0001_130C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_130F,
        end: 0x0001_1310,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1313,
        end: 0x0001_1328,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_132A,
        end: 0x0001_1330,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1332,
        end: 0x0001_1333,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1335,
        end: 0x0001_1339,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_133B,
        end: 0x0001_133C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_133D,
        end: 0x0001_133D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_133E,
        end: 0x0001_133F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1340,
        end: 0x0001_1340,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1341,
        end: 0x0001_1344,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1347,
        end: 0x0001_1348,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_134B,
        end: 0x0001_134D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1350,
        end: 0x0001_1350,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1357,
        end: 0x0001_1357,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_135D,
        end: 0x0001_1361,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1362,
        end: 0x0001_1363,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1366,
        end: 0x0001_136C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1370,
        end: 0x0001_1374,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1380,
        end: 0x0001_1389,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_138B,
        end: 0x0001_138B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_138E,
        end: 0x0001_138E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1390,
        end: 0x0001_13B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13B7,
        end: 0x0001_13B7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13B8,
        end: 0x0001_13BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13BB,
        end: 0x0001_13C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13C2,
        end: 0x0001_13C2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13C5,
        end: 0x0001_13C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13C7,
        end: 0x0001_13CA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13CC,
        end: 0x0001_13CD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13CE,
        end: 0x0001_13CE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13CF,
        end: 0x0001_13CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D0,
        end: 0x0001_13D0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D1,
        end: 0x0001_13D1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D2,
        end: 0x0001_13D2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D3,
        end: 0x0001_13D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D4,
        end: 0x0001_13D5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13D7,
        end: 0x0001_13D8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_13E1,
        end: 0x0001_13E2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1400,
        end: 0x0001_1434,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1435,
        end: 0x0001_1437,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1438,
        end: 0x0001_143F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1440,
        end: 0x0001_1441,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1442,
        end: 0x0001_1444,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1445,
        end: 0x0001_1445,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1446,
        end: 0x0001_1446,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1447,
        end: 0x0001_144A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_144B,
        end: 0x0001_144F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1450,
        end: 0x0001_1459,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_145A,
        end: 0x0001_145B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_145D,
        end: 0x0001_145D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_145E,
        end: 0x0001_145E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_145F,
        end: 0x0001_1461,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1480,
        end: 0x0001_14AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14B0,
        end: 0x0001_14B2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14B3,
        end: 0x0001_14B8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14B9,
        end: 0x0001_14B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14BA,
        end: 0x0001_14BA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14BB,
        end: 0x0001_14BE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14BF,
        end: 0x0001_14C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14C1,
        end: 0x0001_14C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14C2,
        end: 0x0001_14C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14C4,
        end: 0x0001_14C5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14C6,
        end: 0x0001_14C6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14C7,
        end: 0x0001_14C7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_14D0,
        end: 0x0001_14D9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1580,
        end: 0x0001_15AE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15AF,
        end: 0x0001_15B1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15B2,
        end: 0x0001_15B5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15B6,
        end: 0x0001_15B7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15B8,
        end: 0x0001_15BB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15BC,
        end: 0x0001_15BD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15BE,
        end: 0x0001_15BE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15BF,
        end: 0x0001_15C0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15C1,
        end: 0x0001_15D7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15D8,
        end: 0x0001_15DB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15DC,
        end: 0x0001_15DD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_15DE,
        end: 0x0001_15FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1600,
        end: 0x0001_162F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1630,
        end: 0x0001_1632,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1633,
        end: 0x0001_163A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_163B,
        end: 0x0001_163C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_163D,
        end: 0x0001_163D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_163E,
        end: 0x0001_163E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_163F,
        end: 0x0001_1640,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1641,
        end: 0x0001_1643,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1644,
        end: 0x0001_1644,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1650,
        end: 0x0001_1659,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1660,
        end: 0x0001_166C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1680,
        end: 0x0001_16AA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16AB,
        end: 0x0001_16AB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16AC,
        end: 0x0001_16AC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16AD,
        end: 0x0001_16AD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16AE,
        end: 0x0001_16AF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16B0,
        end: 0x0001_16B5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16B6,
        end: 0x0001_16B6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16B7,
        end: 0x0001_16B7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16B8,
        end: 0x0001_16B8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16B9,
        end: 0x0001_16B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16C0,
        end: 0x0001_16C9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_16D0,
        end: 0x0001_16E3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1700,
        end: 0x0001_171A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_171D,
        end: 0x0001_171D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_171E,
        end: 0x0001_171E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_171F,
        end: 0x0001_171F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1720,
        end: 0x0001_1721,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1722,
        end: 0x0001_1725,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1726,
        end: 0x0001_1726,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1727,
        end: 0x0001_172B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1730,
        end: 0x0001_1739,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_173A,
        end: 0x0001_173B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_173C,
        end: 0x0001_173E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_173F,
        end: 0x0001_173F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1740,
        end: 0x0001_1746,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1800,
        end: 0x0001_182B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_182C,
        end: 0x0001_182E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_182F,
        end: 0x0001_1837,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1838,
        end: 0x0001_1838,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1839,
        end: 0x0001_183A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_183B,
        end: 0x0001_183B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_18A0,
        end: 0x0001_18DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_18E0,
        end: 0x0001_18E9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_18EA,
        end: 0x0001_18F2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_18FF,
        end: 0x0001_18FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1900,
        end: 0x0001_1906,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1909,
        end: 0x0001_1909,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_190C,
        end: 0x0001_1913,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1915,
        end: 0x0001_1916,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1918,
        end: 0x0001_192F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1930,
        end: 0x0001_1935,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1937,
        end: 0x0001_1938,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_193B,
        end: 0x0001_193C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_193D,
        end: 0x0001_193D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_193E,
        end: 0x0001_193E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_193F,
        end: 0x0001_193F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1940,
        end: 0x0001_1940,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1941,
        end: 0x0001_1941,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1942,
        end: 0x0001_1942,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1943,
        end: 0x0001_1943,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1944,
        end: 0x0001_1946,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1950,
        end: 0x0001_1959,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19A0,
        end: 0x0001_19A7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19AA,
        end: 0x0001_19D0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19D1,
        end: 0x0001_19D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19D4,
        end: 0x0001_19D7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19DA,
        end: 0x0001_19DB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19DC,
        end: 0x0001_19DF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19E0,
        end: 0x0001_19E0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19E1,
        end: 0x0001_19E1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19E2,
        end: 0x0001_19E2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19E3,
        end: 0x0001_19E3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_19E4,
        end: 0x0001_19E4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1A00,
        end: 0x0001_1A00,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A01,
        end: 0x0001_1A0A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A0B,
        end: 0x0001_1A32,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A33,
        end: 0x0001_1A38,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A39,
        end: 0x0001_1A39,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A3A,
        end: 0x0001_1A3A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A3B,
        end: 0x0001_1A3E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A3F,
        end: 0x0001_1A46,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A47,
        end: 0x0001_1A47,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A48,
        end: 0x0001_1A4F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A50,
        end: 0x0001_1A50,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A51,
        end: 0x0001_1A56,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A57,
        end: 0x0001_1A58,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A59,
        end: 0x0001_1A5B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A5C,
        end: 0x0001_1A89,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A8A,
        end: 0x0001_1A96,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A97,
        end: 0x0001_1A97,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A98,
        end: 0x0001_1A99,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A9A,
        end: 0x0001_1A9C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A9D,
        end: 0x0001_1A9D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1A9E,
        end: 0x0001_1AA2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1AA3,
        end: 0x0001_1AAF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1AB0,
        end: 0x0001_1ABF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_1AC0,
        end: 0x0001_1AF8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B00,
        end: 0x0001_1B09,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B60,
        end: 0x0001_1B60,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B61,
        end: 0x0001_1B61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B62,
        end: 0x0001_1B64,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B65,
        end: 0x0001_1B65,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B66,
        end: 0x0001_1B66,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1B67,
        end: 0x0001_1B67,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1BC0,
        end: 0x0001_1BE0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1BE1,
        end: 0x0001_1BE1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1BF0,
        end: 0x0001_1BF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C00,
        end: 0x0001_1C08,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C0A,
        end: 0x0001_1C2E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C2F,
        end: 0x0001_1C2F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C30,
        end: 0x0001_1C36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C38,
        end: 0x0001_1C3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C3E,
        end: 0x0001_1C3E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C3F,
        end: 0x0001_1C3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C40,
        end: 0x0001_1C40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C41,
        end: 0x0001_1C45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C50,
        end: 0x0001_1C59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C5A,
        end: 0x0001_1C6C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C70,
        end: 0x0001_1C71,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C72,
        end: 0x0001_1C8F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1C92,
        end: 0x0001_1CA7,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CA9,
        end: 0x0001_1CA9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CAA,
        end: 0x0001_1CB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CB1,
        end: 0x0001_1CB1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CB2,
        end: 0x0001_1CB3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CB4,
        end: 0x0001_1CB4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1CB5,
        end: 0x0001_1CB6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D00,
        end: 0x0001_1D06,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D08,
        end: 0x0001_1D09,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D0B,
        end: 0x0001_1D30,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D31,
        end: 0x0001_1D36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D3A,
        end: 0x0001_1D3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D3C,
        end: 0x0001_1D3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D3F,
        end: 0x0001_1D45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D46,
        end: 0x0001_1D46,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D47,
        end: 0x0001_1D47,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D50,
        end: 0x0001_1D59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D60,
        end: 0x0001_1D65,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D67,
        end: 0x0001_1D68,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D6A,
        end: 0x0001_1D89,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D8A,
        end: 0x0001_1D8E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D90,
        end: 0x0001_1D91,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D93,
        end: 0x0001_1D94,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D95,
        end: 0x0001_1D95,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D96,
        end: 0x0001_1D96,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D97,
        end: 0x0001_1D97,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1D98,
        end: 0x0001_1D98,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1DA0,
        end: 0x0001_1DA9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1DB0,
        end: 0x0001_1DD8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1DD9,
        end: 0x0001_1DD9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1DDA,
        end: 0x0001_1DDB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1DE0,
        end: 0x0001_1DE9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1EE0,
        end: 0x0001_1EF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1EF3,
        end: 0x0001_1EF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1EF5,
        end: 0x0001_1EF6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1EF7,
        end: 0x0001_1EF8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F00,
        end: 0x0001_1F01,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F02,
        end: 0x0001_1F02,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F03,
        end: 0x0001_1F03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F04,
        end: 0x0001_1F10,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F12,
        end: 0x0001_1F33,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F34,
        end: 0x0001_1F35,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F36,
        end: 0x0001_1F3A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F3E,
        end: 0x0001_1F3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F40,
        end: 0x0001_1F40,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F41,
        end: 0x0001_1F41,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F42,
        end: 0x0001_1F42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F43,
        end: 0x0001_1F4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F50,
        end: 0x0001_1F59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1F5A,
        end: 0x0001_1F5A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FB0,
        end: 0x0001_1FB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FC0,
        end: 0x0001_1FD4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FD5,
        end: 0x0001_1FDC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FDD,
        end: 0x0001_1FE0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FE1,
        end: 0x0001_1FF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_1FFF,
        end: 0x0001_1FFF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2000,
        end: 0x0001_2399,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2400,
        end: 0x0001_246E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2470,
        end: 0x0001_2474,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2480,
        end: 0x0001_2543,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2F90,
        end: 0x0001_2FF0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_2FF1,
        end: 0x0001_2FF2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_3000,
        end: 0x0001_342F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3430,
        end: 0x0001_343F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3440,
        end: 0x0001_3440,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3441,
        end: 0x0001_3446,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3447,
        end: 0x0001_3455,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3456,
        end: 0x0001_345F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_3460,
        end: 0x0001_43FA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_43FB,
        end: 0x0001_43FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_4400,
        end: 0x0001_4646,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_4647,
        end: 0x0001_467F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6100,
        end: 0x0001_611D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_611E,
        end: 0x0001_6129,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_612A,
        end: 0x0001_612C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_612D,
        end: 0x0001_612F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6130,
        end: 0x0001_6139,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6800,
        end: 0x0001_6A38,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6A40,
        end: 0x0001_6A5E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6A60,
        end: 0x0001_6A69,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6A6E,
        end: 0x0001_6A6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6A70,
        end: 0x0001_6ABE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6AC0,
        end: 0x0001_6AC9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6AD0,
        end: 0x0001_6AED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6AF0,
        end: 0x0001_6AF4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6AF5,
        end: 0x0001_6AF5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B00,
        end: 0x0001_6B2F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B30,
        end: 0x0001_6B36,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B37,
        end: 0x0001_6B3B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B3C,
        end: 0x0001_6B3F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B40,
        end: 0x0001_6B43,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B44,
        end: 0x0001_6B44,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B45,
        end: 0x0001_6B45,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B50,
        end: 0x0001_6B59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B5B,
        end: 0x0001_6B61,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B63,
        end: 0x0001_6B77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6B7D,
        end: 0x0001_6B8F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6D40,
        end: 0x0001_6D42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6D43,
        end: 0x0001_6D6A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6D6B,
        end: 0x0001_6D6C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6D6D,
        end: 0x0001_6D6F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6D70,
        end: 0x0001_6D79,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6E40,
        end: 0x0001_6E7F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6E80,
        end: 0x0001_6E96,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6E97,
        end: 0x0001_6E9A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6EA0,
        end: 0x0001_6EB8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6EBB,
        end: 0x0001_6ED3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F00,
        end: 0x0001_6F4A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F4F,
        end: 0x0001_6F4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F50,
        end: 0x0001_6F50,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F51,
        end: 0x0001_6F87,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F8F,
        end: 0x0001_6F92,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6F93,
        end: 0x0001_6F9F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_6FE0,
        end: 0x0001_6FE1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FE2,
        end: 0x0001_6FE2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FE3,
        end: 0x0001_6FE3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FE4,
        end: 0x0001_6FE4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FE5,
        end: 0x0001_6FEF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FF0,
        end: 0x0001_6FF1,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FF2,
        end: 0x0001_6FF3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FF4,
        end: 0x0001_6FF6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_6FF7,
        end: 0x0001_6FFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_7000,
        end: 0x0001_87FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8800,
        end: 0x0001_8AFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8B00,
        end: 0x0001_8CD5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8CD6,
        end: 0x0001_8CFE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8CFF,
        end: 0x0001_8CFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8D00,
        end: 0x0001_8D1E,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8D1F,
        end: 0x0001_8D7F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8D80,
        end: 0x0001_8DF2,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_8DF3,
        end: 0x0001_8DFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFF0,
        end: 0x0001_AFF3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFF4,
        end: 0x0001_AFF4,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFF5,
        end: 0x0001_AFFB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFFC,
        end: 0x0001_AFFC,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFFD,
        end: 0x0001_AFFE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_AFFF,
        end: 0x0001_AFFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B000,
        end: 0x0001_B0FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B100,
        end: 0x0001_B122,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B123,
        end: 0x0001_B12F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B130,
        end: 0x0001_B131,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B132,
        end: 0x0001_B132,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x0001_B133,
        end: 0x0001_B14F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B150,
        end: 0x0001_B152,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x0001_B153,
        end: 0x0001_B154,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B155,
        end: 0x0001_B155,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x0001_B156,
        end: 0x0001_B163,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B164,
        end: 0x0001_B167,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x0001_B168,
        end: 0x0001_B16F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B170,
        end: 0x0001_B2FB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_B2FC,
        end: 0x0001_B2FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_BC00,
        end: 0x0001_BC6A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC70,
        end: 0x0001_BC7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC80,
        end: 0x0001_BC88,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC90,
        end: 0x0001_BC99,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC9C,
        end: 0x0001_BC9C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC9D,
        end: 0x0001_BC9E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BC9F,
        end: 0x0001_BC9F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_BCA0,
        end: 0x0001_BCA3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CC00,
        end: 0x0001_CCEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CCF0,
        end: 0x0001_CCF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CCFA,
        end: 0x0001_CCFC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CD00,
        end: 0x0001_CEB3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CEBA,
        end: 0x0001_CEBF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_CEC0,
        end: 0x0001_CED0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CED1,
        end: 0x0001_CEDF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CEE0,
        end: 0x0001_CEEF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CEF0,
        end: 0x0001_CEF0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CEF1,
        end: 0x0001_CEFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CF00,
        end: 0x0001_CF2D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CF2E,
        end: 0x0001_CF2F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CF30,
        end: 0x0001_CF46,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CF47,
        end: 0x0001_CF4F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CF50,
        end: 0x0001_CFC3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_CFC4,
        end: 0x0001_CFCF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D000,
        end: 0x0001_D0F5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D0F6,
        end: 0x0001_D0FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D100,
        end: 0x0001_D126,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D127,
        end: 0x0001_D128,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D129,
        end: 0x0001_D164,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D165,
        end: 0x0001_D166,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D167,
        end: 0x0001_D169,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D16A,
        end: 0x0001_D16C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D16D,
        end: 0x0001_D172,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D173,
        end: 0x0001_D17A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D17B,
        end: 0x0001_D182,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D183,
        end: 0x0001_D184,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D185,
        end: 0x0001_D18B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D18C,
        end: 0x0001_D1A9,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D1AA,
        end: 0x0001_D1AD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D1AE,
        end: 0x0001_D1EA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D1EB,
        end: 0x0001_D1FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D200,
        end: 0x0001_D241,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D242,
        end: 0x0001_D244,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D245,
        end: 0x0001_D245,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D2C0,
        end: 0x0001_D2D3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D2E0,
        end: 0x0001_D2F3,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D2F4,
        end: 0x0001_D2FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D300,
        end: 0x0001_D356,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D357,
        end: 0x0001_D35F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D360,
        end: 0x0001_D378,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D379,
        end: 0x0001_D37F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_D400,
        end: 0x0001_D454,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D456,
        end: 0x0001_D49C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D49E,
        end: 0x0001_D49F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4A2,
        end: 0x0001_D4A2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4A5,
        end: 0x0001_D4A6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4A9,
        end: 0x0001_D4AC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4AE,
        end: 0x0001_D4B9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4BB,
        end: 0x0001_D4BB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4BD,
        end: 0x0001_D4C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D4C5,
        end: 0x0001_D505,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D507,
        end: 0x0001_D50A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D50D,
        end: 0x0001_D514,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D516,
        end: 0x0001_D51C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D51E,
        end: 0x0001_D539,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D53B,
        end: 0x0001_D53E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D540,
        end: 0x0001_D544,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D546,
        end: 0x0001_D546,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D54A,
        end: 0x0001_D550,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D552,
        end: 0x0001_D6A5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6A8,
        end: 0x0001_D6C0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6C1,
        end: 0x0001_D6C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6C2,
        end: 0x0001_D6DA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6DB,
        end: 0x0001_D6DB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6DC,
        end: 0x0001_D6FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6FB,
        end: 0x0001_D6FB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D6FC,
        end: 0x0001_D714,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D715,
        end: 0x0001_D715,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D716,
        end: 0x0001_D734,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D735,
        end: 0x0001_D735,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D736,
        end: 0x0001_D74E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D74F,
        end: 0x0001_D74F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D750,
        end: 0x0001_D76E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D76F,
        end: 0x0001_D76F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D770,
        end: 0x0001_D788,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D789,
        end: 0x0001_D789,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D78A,
        end: 0x0001_D7A8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D7A9,
        end: 0x0001_D7A9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D7AA,
        end: 0x0001_D7C2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D7C3,
        end: 0x0001_D7C3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D7C4,
        end: 0x0001_D7CB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D7CE,
        end: 0x0001_D7FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_D800,
        end: 0x0001_D9FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA00,
        end: 0x0001_DA36,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA37,
        end: 0x0001_DA3A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA3B,
        end: 0x0001_DA6C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA6D,
        end: 0x0001_DA74,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA75,
        end: 0x0001_DA75,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA76,
        end: 0x0001_DA83,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA84,
        end: 0x0001_DA84,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA85,
        end: 0x0001_DA86,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA87,
        end: 0x0001_DA8B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA8C,
        end: 0x0001_DA9A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DA9B,
        end: 0x0001_DA9F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DAA0,
        end: 0x0001_DAA0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DAA1,
        end: 0x0001_DAAF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_DF00,
        end: 0x0001_DF09,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_DF0A,
        end: 0x0001_DF0A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_DF0B,
        end: 0x0001_DF1E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_DF25,
        end: 0x0001_DF2A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E000,
        end: 0x0001_E006,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E008,
        end: 0x0001_E018,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E01B,
        end: 0x0001_E021,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E023,
        end: 0x0001_E024,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E026,
        end: 0x0001_E02A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E030,
        end: 0x0001_E06D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E08F,
        end: 0x0001_E08F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E100,
        end: 0x0001_E12C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E130,
        end: 0x0001_E136,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E137,
        end: 0x0001_E13D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E140,
        end: 0x0001_E149,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E14E,
        end: 0x0001_E14E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E14F,
        end: 0x0001_E14F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E290,
        end: 0x0001_E2AD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E2AE,
        end: 0x0001_E2AE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E2C0,
        end: 0x0001_E2EB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E2EC,
        end: 0x0001_E2EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E2F0,
        end: 0x0001_E2F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E2FF,
        end: 0x0001_E2FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E4D0,
        end: 0x0001_E4EA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E4EB,
        end: 0x0001_E4EB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E4EC,
        end: 0x0001_E4EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E4F0,
        end: 0x0001_E4F9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E5D0,
        end: 0x0001_E5ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E5EE,
        end: 0x0001_E5EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E5F0,
        end: 0x0001_E5F0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E5F1,
        end: 0x0001_E5FA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E5FF,
        end: 0x0001_E5FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6C0,
        end: 0x0001_E6DE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6E0,
        end: 0x0001_E6E2,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6E3,
        end: 0x0001_E6E3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6E4,
        end: 0x0001_E6E5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6E6,
        end: 0x0001_E6E6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6E7,
        end: 0x0001_E6ED,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6EE,
        end: 0x0001_E6EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6F0,
        end: 0x0001_E6F4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6F5,
        end: 0x0001_E6F5,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6FE,
        end: 0x0001_E6FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E6FF,
        end: 0x0001_E6FF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E7E0,
        end: 0x0001_E7E6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E7E8,
        end: 0x0001_E7EB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E7ED,
        end: 0x0001_E7EE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E7F0,
        end: 0x0001_E7FE,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E800,
        end: 0x0001_E8C4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E8C7,
        end: 0x0001_E8CF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E8D0,
        end: 0x0001_E8D6,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E900,
        end: 0x0001_E943,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E944,
        end: 0x0001_E94A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E94B,
        end: 0x0001_E94B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E950,
        end: 0x0001_E959,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_E95E,
        end: 0x0001_E95F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EC71,
        end: 0x0001_ECAB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ECAC,
        end: 0x0001_ECAC,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ECAD,
        end: 0x0001_ECAF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ECB0,
        end: 0x0001_ECB0,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ECB1,
        end: 0x0001_ECB4,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ED01,
        end: 0x0001_ED2D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ED2E,
        end: 0x0001_ED2E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_ED2F,
        end: 0x0001_ED3D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE00,
        end: 0x0001_EE03,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE05,
        end: 0x0001_EE1F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE21,
        end: 0x0001_EE22,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE24,
        end: 0x0001_EE24,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE27,
        end: 0x0001_EE27,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE29,
        end: 0x0001_EE32,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE34,
        end: 0x0001_EE37,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE39,
        end: 0x0001_EE39,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE3B,
        end: 0x0001_EE3B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE42,
        end: 0x0001_EE42,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE47,
        end: 0x0001_EE47,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE49,
        end: 0x0001_EE49,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE4B,
        end: 0x0001_EE4B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE4D,
        end: 0x0001_EE4F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE51,
        end: 0x0001_EE52,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE54,
        end: 0x0001_EE54,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE57,
        end: 0x0001_EE57,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE59,
        end: 0x0001_EE59,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE5B,
        end: 0x0001_EE5B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE5D,
        end: 0x0001_EE5D,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE5F,
        end: 0x0001_EE5F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE61,
        end: 0x0001_EE62,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE64,
        end: 0x0001_EE64,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE67,
        end: 0x0001_EE6A,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE6C,
        end: 0x0001_EE72,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE74,
        end: 0x0001_EE77,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE79,
        end: 0x0001_EE7C,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE7E,
        end: 0x0001_EE7E,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE80,
        end: 0x0001_EE89,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EE8B,
        end: 0x0001_EE9B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EEA1,
        end: 0x0001_EEA3,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EEA5,
        end: 0x0001_EEA9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EEAB,
        end: 0x0001_EEBB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_EEF0,
        end: 0x0001_EEF1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F000,
        end: 0x0001_F02B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F02C,
        end: 0x0001_F02F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F030,
        end: 0x0001_F093,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F094,
        end: 0x0001_F09F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0A0,
        end: 0x0001_F0AE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0AF,
        end: 0x0001_F0B0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0B1,
        end: 0x0001_F0BF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0C0,
        end: 0x0001_F0C0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0C1,
        end: 0x0001_F0CF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0D0,
        end: 0x0001_F0D0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0D1,
        end: 0x0001_F0F5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F0F6,
        end: 0x0001_F0FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F100,
        end: 0x0001_F10C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F10D,
        end: 0x0001_F1AD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F1AE,
        end: 0x0001_F1E5,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F1E6,
        end: 0x0001_F1FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F200,
        end: 0x0001_F201,
        orientation: UnicodeVerticalOrientation::TransformedUpright,
    },
    VerticalOrientationRange {
        start: 0x0001_F202,
        end: 0x0001_F202,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F203,
        end: 0x0001_F20F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F210,
        end: 0x0001_F23B,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F23C,
        end: 0x0001_F23F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F240,
        end: 0x0001_F248,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F249,
        end: 0x0001_F24F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F250,
        end: 0x0001_F251,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F252,
        end: 0x0001_F25F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F260,
        end: 0x0001_F265,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F266,
        end: 0x0001_F2FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F300,
        end: 0x0001_F3FA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F3FB,
        end: 0x0001_F3FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F400,
        end: 0x0001_F5FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F600,
        end: 0x0001_F64F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F650,
        end: 0x0001_F67F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F680,
        end: 0x0001_F6D8,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F6D9,
        end: 0x0001_F6DB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F6DC,
        end: 0x0001_F6EC,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F6ED,
        end: 0x0001_F6EF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F6F0,
        end: 0x0001_F6FC,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F6FD,
        end: 0x0001_F6FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F700,
        end: 0x0001_F77F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F780,
        end: 0x0001_F7D9,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F7DA,
        end: 0x0001_F7DF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F7E0,
        end: 0x0001_F7EB,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F7EC,
        end: 0x0001_F7EF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F7F0,
        end: 0x0001_F7F0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F7F1,
        end: 0x0001_F7FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_F800,
        end: 0x0001_F80B,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F810,
        end: 0x0001_F847,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F850,
        end: 0x0001_F859,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F860,
        end: 0x0001_F887,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F890,
        end: 0x0001_F8AD,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F8B0,
        end: 0x0001_F8BB,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F8C0,
        end: 0x0001_F8C1,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F8D0,
        end: 0x0001_F8D8,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_F900,
        end: 0x0001_F9FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA00,
        end: 0x0001_FA57,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA58,
        end: 0x0001_FA5F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA60,
        end: 0x0001_FA6D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA6E,
        end: 0x0001_FA6F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA70,
        end: 0x0001_FA7C,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA7D,
        end: 0x0001_FA7F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA80,
        end: 0x0001_FA8A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA8B,
        end: 0x0001_FA8D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FA8E,
        end: 0x0001_FAC6,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAC7,
        end: 0x0001_FAC7,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAC8,
        end: 0x0001_FAC8,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAC9,
        end: 0x0001_FACC,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FACD,
        end: 0x0001_FADC,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FADD,
        end: 0x0001_FADE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FADF,
        end: 0x0001_FAEA,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAEB,
        end: 0x0001_FAEE,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAEF,
        end: 0x0001_FAF8,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FAF9,
        end: 0x0001_FAFF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0001_FB00,
        end: 0x0001_FB92,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_FB94,
        end: 0x0001_FBEF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_FBF0,
        end: 0x0001_FBF9,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0001_FBFA,
        end: 0x0001_FBFA,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x0002_0000,
        end: 0x0002_A6DF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_A6E0,
        end: 0x0002_A6FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_A700,
        end: 0x0002_B81D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_B81E,
        end: 0x0002_B81F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_B820,
        end: 0x0002_CEAD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_CEAE,
        end: 0x0002_CEAF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_CEB0,
        end: 0x0002_EBE0,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_EBE1,
        end: 0x0002_EBEF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_EBF0,
        end: 0x0002_EE5D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_EE5E,
        end: 0x0002_F7FF,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_F800,
        end: 0x0002_FA1D,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_FA1E,
        end: 0x0002_FA1F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0002_FA20,
        end: 0x0002_FFFD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0003_0000,
        end: 0x0003_134A,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0003_134B,
        end: 0x0003_134F,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0003_1350,
        end: 0x0003_3479,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0003_347A,
        end: 0x0003_FFFD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x000E_0001,
        end: 0x000E_0001,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x000E_0020,
        end: 0x000E_007F,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x000E_0100,
        end: 0x000E_01EF,
        orientation: UnicodeVerticalOrientation::Rotated,
    },
    VerticalOrientationRange {
        start: 0x000F_0000,
        end: 0x000F_FFFD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
    VerticalOrientationRange {
        start: 0x0010_0000,
        end: 0x0010_FFFD,
        orientation: UnicodeVerticalOrientation::Upright,
    },
];
