use plist::{Dictionary, Value};

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct StickyColorScheme {
    pub control: Dictionary,
    pub highlight: Dictionary,
    pub spine: Dictionary,
    pub sticky: Dictionary,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickyColorPreset {
    Blue,
}

impl StickyColorPreset {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn scheme(self) -> StickyColorScheme {
        match self {
            Self::Blue => StickyColorScheme {
                control: color_dictionary(0.1411764705882353, 0.8156862745098039, 0.9137254901960784, 1.0),
                highlight: color_dictionary(0.00784313725490196, 0.7372549019607844, 0.8431372549019608, 1.0),
                spine: color_dictionary(0.5372549019607843, 0.9411764705882353, 1.0, 1.0),
                sticky: color_dictionary(0.6784313725490196, 0.9568627450980393, 1.0, 1.0),
            },
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn color_dictionary(red: f64, green: f64, blue: f64, alpha: f64) -> Dictionary {
    let mut raw = Dictionary::new();
    raw.insert("Red".into(), Value::Real(red));
    raw.insert("Green".into(), Value::Real(green));
    raw.insert("Blue".into(), Value::Real(blue));
    raw.insert("Alpha".into(), Value::Real(alpha));
    raw
}
