//! Checked-in JLREQ punctuation range data.
//!
//! This file is generated from `../data/jlreq_punctuation_ranges.txt`
//! and `../data/jlreq_pair_adjustments.txt`.
//! Do not edit range data by hand; run `tools/generate_jlreq_punctuation_data.rs`.

/// Checked-in JLREQ punctuation data version.
pub const JLREQ_PUNCTUATION_DATA_VERSION: &str = "arcweft-jlreq-punctuation-2026-06-12";

/// Checked-in JLREQ pair adjustment data version.
pub const JLREQ_PAIR_ADJUSTMENT_DATA_VERSION: &str =
    "arcweft-jlreq-pair-adjustment-2026-06-12-strictness";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JlreqPunctuationClass {
    Closing,
    Opening,
    SmallKana,
    Dash,
    Leader,
    MiddleDot,
    RepeatMark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JlreqPunctuationRange {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) class: JlreqPunctuationClass,
}

pub(crate) const JLREQ_PUNCTUATION_RANGES: &[JlreqPunctuationRange] = &[
    JlreqPunctuationRange {
        start: 0x0028,
        end: 0x0028,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x0029,
        end: 0x0029,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x005B,
        end: 0x005B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x005D,
        end: 0x005D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x007B,
        end: 0x007B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x007D,
        end: 0x007D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x00B7,
        end: 0x00B7,
        class: JlreqPunctuationClass::MiddleDot,
    },
    JlreqPunctuationRange {
        start: 0x2010,
        end: 0x2015,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x2018,
        end: 0x2018,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2019,
        end: 0x2019,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x201C,
        end: 0x201C,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x201D,
        end: 0x201D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2025,
        end: 0x2026,
        class: JlreqPunctuationClass::Leader,
    },
    JlreqPunctuationRange {
        start: 0x203C,
        end: 0x203C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2045,
        end: 0x2045,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2046,
        end: 0x2049,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x207D,
        end: 0x207D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x207E,
        end: 0x207E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x208D,
        end: 0x208D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x208E,
        end: 0x208E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2212,
        end: 0x2212,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x22EF,
        end: 0x22EF,
        class: JlreqPunctuationClass::Leader,
    },
    JlreqPunctuationRange {
        start: 0x2329,
        end: 0x2329,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x232A,
        end: 0x232A,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2500,
        end: 0x2500,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x2768,
        end: 0x2768,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2769,
        end: 0x2769,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x276A,
        end: 0x276A,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x276B,
        end: 0x276B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x276C,
        end: 0x276C,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x276D,
        end: 0x276D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x276E,
        end: 0x276E,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x276F,
        end: 0x276F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2770,
        end: 0x2770,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2771,
        end: 0x2771,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2772,
        end: 0x2772,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2773,
        end: 0x2773,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2774,
        end: 0x2774,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2775,
        end: 0x2775,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27C5,
        end: 0x27C5,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27C6,
        end: 0x27C6,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27E6,
        end: 0x27E6,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27E7,
        end: 0x27E7,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27E8,
        end: 0x27E8,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27E9,
        end: 0x27E9,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27EA,
        end: 0x27EA,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27EB,
        end: 0x27EB,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27EC,
        end: 0x27EC,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27ED,
        end: 0x27ED,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x27EE,
        end: 0x27EE,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x27EF,
        end: 0x27EF,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2983,
        end: 0x2983,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2984,
        end: 0x2984,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2985,
        end: 0x2985,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2986,
        end: 0x2986,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2987,
        end: 0x2987,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2988,
        end: 0x2988,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2989,
        end: 0x2989,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x298A,
        end: 0x298A,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x298B,
        end: 0x298B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x298C,
        end: 0x298C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x298D,
        end: 0x298D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x298E,
        end: 0x298E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x298F,
        end: 0x298F,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2990,
        end: 0x2990,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2991,
        end: 0x2991,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2992,
        end: 0x2992,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2993,
        end: 0x2993,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2994,
        end: 0x2994,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2995,
        end: 0x2995,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2996,
        end: 0x2996,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2997,
        end: 0x2997,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2998,
        end: 0x2998,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x29D8,
        end: 0x29D8,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x29D9,
        end: 0x29D9,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x29DA,
        end: 0x29DA,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x29DB,
        end: 0x29DB,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x29FC,
        end: 0x29FC,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x29FD,
        end: 0x29FD,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2E22,
        end: 0x2E22,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2E23,
        end: 0x2E23,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2E24,
        end: 0x2E24,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2E25,
        end: 0x2E25,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2E26,
        end: 0x2E26,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2E27,
        end: 0x2E27,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x2E28,
        end: 0x2E28,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x2E29,
        end: 0x2E29,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3001,
        end: 0x3002,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3005,
        end: 0x3005,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x3008,
        end: 0x3008,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3009,
        end: 0x3009,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300A,
        end: 0x300A,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300B,
        end: 0x300B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300C,
        end: 0x300C,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300D,
        end: 0x300D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x300E,
        end: 0x300E,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x300F,
        end: 0x300F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3010,
        end: 0x3010,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3011,
        end: 0x3011,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3014,
        end: 0x3014,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3015,
        end: 0x3015,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3016,
        end: 0x3016,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3017,
        end: 0x3017,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3018,
        end: 0x3018,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x3019,
        end: 0x3019,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x301A,
        end: 0x301A,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x301B,
        end: 0x301B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x301C,
        end: 0x301C,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x301D,
        end: 0x301D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0x301E,
        end: 0x301F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0x3030,
        end: 0x3030,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x3031,
        end: 0x3035,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x303B,
        end: 0x303B,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x3041,
        end: 0x3041,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3043,
        end: 0x3043,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3045,
        end: 0x3045,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3047,
        end: 0x3047,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3049,
        end: 0x3049,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3063,
        end: 0x3063,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3083,
        end: 0x3083,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3085,
        end: 0x3085,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3087,
        end: 0x3087,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x308E,
        end: 0x308E,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x3095,
        end: 0x3096,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x309D,
        end: 0x309F,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x30A1,
        end: 0x30A1,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A3,
        end: 0x30A3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A5,
        end: 0x30A5,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A7,
        end: 0x30A7,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30A9,
        end: 0x30A9,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30C3,
        end: 0x30C3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E3,
        end: 0x30E3,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E5,
        end: 0x30E5,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30E7,
        end: 0x30E7,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30EE,
        end: 0x30EE,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30F5,
        end: 0x30F6,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0x30FB,
        end: 0x30FB,
        class: JlreqPunctuationClass::MiddleDot,
    },
    JlreqPunctuationRange {
        start: 0x30FC,
        end: 0x30FC,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0x30FD,
        end: 0x30FF,
        class: JlreqPunctuationClass::RepeatMark,
    },
    JlreqPunctuationRange {
        start: 0x31F0,
        end: 0x31FF,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0xFE19,
        end: 0xFE19,
        class: JlreqPunctuationClass::Leader,
    },
    JlreqPunctuationRange {
        start: 0xFE30,
        end: 0xFE30,
        class: JlreqPunctuationClass::Leader,
    },
    JlreqPunctuationRange {
        start: 0xFE35,
        end: 0xFE35,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE36,
        end: 0xFE36,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE37,
        end: 0xFE37,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE38,
        end: 0xFE38,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE39,
        end: 0xFE39,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE3A,
        end: 0xFE3A,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE3B,
        end: 0xFE3B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE3C,
        end: 0xFE3C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE3D,
        end: 0xFE3D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE3E,
        end: 0xFE3E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE3F,
        end: 0xFE3F,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE40,
        end: 0xFE40,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE41,
        end: 0xFE41,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE42,
        end: 0xFE42,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE43,
        end: 0xFE43,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE44,
        end: 0xFE44,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE47,
        end: 0xFE47,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE48,
        end: 0xFE48,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE59,
        end: 0xFE59,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE5A,
        end: 0xFE5A,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE5B,
        end: 0xFE5B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE5C,
        end: 0xFE5C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFE5D,
        end: 0xFE5D,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFE5E,
        end: 0xFE5E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF01,
        end: 0xFF01,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF08,
        end: 0xFF08,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF09,
        end: 0xFF09,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF0C,
        end: 0xFF0C,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF0D,
        end: 0xFF0D,
        class: JlreqPunctuationClass::Dash,
    },
    JlreqPunctuationRange {
        start: 0xFF0E,
        end: 0xFF0E,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF1A,
        end: 0xFF1B,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF1F,
        end: 0xFF1F,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF3B,
        end: 0xFF3B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF3D,
        end: 0xFF3D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF5B,
        end: 0xFF5B,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF5D,
        end: 0xFF5D,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF5F,
        end: 0xFF5F,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF60,
        end: 0xFF61,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF62,
        end: 0xFF62,
        class: JlreqPunctuationClass::Opening,
    },
    JlreqPunctuationRange {
        start: 0xFF63,
        end: 0xFF64,
        class: JlreqPunctuationClass::Closing,
    },
    JlreqPunctuationRange {
        start: 0xFF65,
        end: 0xFF65,
        class: JlreqPunctuationClass::MiddleDot,
    },
    JlreqPunctuationRange {
        start: 0xFF67,
        end: 0xFF6F,
        class: JlreqPunctuationClass::SmallKana,
    },
    JlreqPunctuationRange {
        start: 0xFF70,
        end: 0xFF70,
        class: JlreqPunctuationClass::Dash,
    },
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct JlreqPairAdjustment {
    pub(crate) keep_together: bool,
    pub(crate) break_penalty: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct JlreqPairAdjustmentSet {
    pub(crate) loose: JlreqPairAdjustment,
    pub(crate) normal: JlreqPairAdjustment,
    pub(crate) strict: JlreqPairAdjustment,
}

impl JlreqPairAdjustmentSet {
    pub(crate) const fn for_strictness(
        self,
        strictness: crate::JlreqStrictness,
    ) -> JlreqPairAdjustment {
        match strictness {
            crate::JlreqStrictness::Loose => self.loose,
            crate::JlreqStrictness::Normal => self.normal,
            crate::JlreqStrictness::Strict => self.strict,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JlreqPairAdjustmentRule {
    pub(crate) left: Option<JlreqPunctuationClass>,
    pub(crate) right: JlreqPunctuationClass,
    pub(crate) adjustments: JlreqPairAdjustmentSet,
}

pub(crate) const JLREQ_PAIR_ADJUSTMENTS: &[JlreqPairAdjustmentRule] = &[
    JlreqPairAdjustmentRule {
        left: None,
        right: JlreqPunctuationClass::RepeatMark,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
            normal: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
            strict: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Dash),
        right: JlreqPunctuationClass::Dash,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 100,
            },
            normal: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
            strict: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Leader),
        right: JlreqPunctuationClass::Leader,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 100,
            },
            normal: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
            strict: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Closing),
        right: JlreqPunctuationClass::Opening,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 5,
            },
            normal: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 25,
            },
            strict: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 100,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::Closing),
        right: JlreqPunctuationClass::Closing,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 5,
            },
            normal: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 20,
            },
            strict: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 75,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::MiddleDot),
        right: JlreqPunctuationClass::Opening,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 0,
            },
            normal: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 15,
            },
            strict: JlreqPairAdjustment {
                keep_together: true,
                break_penalty: 1000,
            },
        },
    },
    JlreqPairAdjustmentRule {
        left: Some(JlreqPunctuationClass::MiddleDot),
        right: JlreqPunctuationClass::Closing,
        adjustments: JlreqPairAdjustmentSet {
            loose: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 0,
            },
            normal: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 10,
            },
            strict: JlreqPairAdjustment {
                keep_together: false,
                break_penalty: 50,
            },
        },
    },
];
